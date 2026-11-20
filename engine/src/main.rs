//! Audiomeda engine: headless matrix mixer. Owns the state, captures every app that
//! plays sound (routing its original output to the silent sink) and mixes it to each output.

mod audio;
mod state;
mod win;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use audio::{Conns, Source, Worker};
use state::{State, app_key};
use wasapi::{Direction, initialize_mta};
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows_core::BOOL;
use win::Res;

static STOP: AtomicBool = AtomicBool::new(false);
static DONE: AtomicBool = AtomicBool::new(false);

/// Ctrl+C / closing the console: let the main loop un-route apps before the process dies.
unsafe extern "system" fn on_ctrl(_: u32) -> BOOL {
    STOP.store(true, Ordering::Relaxed);
    for _ in 0..40 {
        if DONE.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    BOOL(1)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn first_run_state() -> Res<State> {
    let renders = win::devices(Direction::Render)?;
    // VB-Cable D is the sink on this setup; change "sink" in state.json for another device
    let sink = renders
        .iter()
        .find(|(_, name)| name.contains("CABLE-D Input"))
        .map(|(id, _)| id.clone())
        .ok_or("no \"CABLE-D Input\" device for the sink; install VB-Cable C+D or edit state.json")?;
    let (primary, name) = win::default_render()?;
    if primary == sink {
        return Err("the Windows default output is the sink cable; pick your headphones as default first".into());
    }
    Ok(State::new(sink, primary, name))
}

fn run() -> Res<()> {
    initialize_mta().ok()?;
    unsafe { SetConsoleCtrlHandler(Some(on_ctrl), true)? };

    let path = State::path();
    let state = match State::load(&path)? {
        Some(s) => s,
        None => {
            let s = first_run_state()?;
            s.save(&path)?;
            s
        }
    };
    println!("state: {}", path.display());

    // `reset`: after a crash or kill, give every known app its normal output back
    if std::env::args().nth(1).as_deref() == Some("reset") {
        let procs = win::processes()?;
        for key in state.inputs.keys().filter_map(|k| k.strip_prefix("app:")) {
            if win::route(&procs, key, None).is_ok() {
                println!("{key} returned to its normal output");
            }
        }
        return Ok(());
    }
    let sink = state.sink.clone();
    let state = Arc::new(Mutex::new(state));
    // shortcut: hand edits to state.json are picked up by polling its mtime; the named pipe replaces this
    let mtime = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut seen = mtime(&path);
    let conns: Conns = Default::default();
    let own_pid = std::process::id();

    let mut apps: HashMap<String, (u32, Worker)> = HashMap::new(); // exe -> (root pid, capture)
    let mut mics: HashMap<String, Worker> = HashMap::new(); // input key -> capture
    let mut outs: HashMap<String, Worker> = HashMap::new(); // device id -> render

    // shortcut: polls every 500 ms instead of device/session notifications; switch if the
    // half-second delay before a new app is heard ever matters.
    while !STOP.load(Ordering::Relaxed) {
        if mtime(&path) != seen {
            seen = mtime(&path);
            match State::load(&path) {
                Ok(Some(s)) => {
                    if s.sink != sink {
                        println!("sink changed in state.json: restart the engine to apply it");
                    }
                    *state.lock().unwrap() = s;
                    println!("reloaded state.json");
                }
                Ok(None) => {}
                Err(e) => println!("state.json not reloaded, keeping current settings: {e}"),
            }
        }
        let procs = win::processes()?;
        let renders = win::devices(Direction::Render)?;
        let captures = win::devices(Direction::Capture)?;
        let mut dirty = false;

        // apps playing anywhere but the sink are new or escaped (e.g. restarted): route + manage
        let by_pid = win::exe_by_pid(&procs);
        let playing: HashSet<String> = win::playing_pids(&[sink.as_str()])?
            .into_iter()
            .filter(|&pid| pid != 0 && pid != own_pid)
            .filter_map(|pid| by_pid.get(&pid).map(|e| e.to_string()))
            .collect();
        for exe in &playing {
            dirty |= state.lock().unwrap().ensure_app(exe);
            // re-sent every tick until the app actually moves (some only move when playback restarts)
            let r = win::route(&procs, exe, Some(&sink));
            if !apps.contains_key(exe) {
                match r {
                    Ok(()) => println!("routed {exe} to sink"),
                    Err(e) => println!("can't route {exe}, it will be heard twice: {e}"),
                }
            }
        }

        // every known app that is running gets a capture (restarted if it died or the app restarted)
        let known: Vec<String> = state
            .lock()
            .unwrap()
            .inputs
            .keys()
            .filter_map(|k| k.strip_prefix("app:").map(str::to_string))
            .collect();
        for exe in known {
            let Some(root) = win::root_pid(&procs, &exe) else {
                if apps.remove(&exe).is_some() {
                    println!("{exe} exited");
                }
                continue;
            };
            if apps.get(&exe).is_some_and(|(pid, w)| *pid == root && w.alive()) {
                continue;
            }
            if !apps.contains_key(&exe) && !playing.contains(&exe) {
                // known app running but still routed to sink from last time: make sure it is
                let _ = win::route(&procs, &exe, Some(&sink));
            }
            println!("capturing {exe} (pid {root})");
            let (key, c) = (app_key(&exe), conns.clone());
            apps.insert(exe.clone(), (root, Worker::spawn(exe, move |stop| audio::capture(key, Source::App(root), c, stop))));
        }

        let managed: HashSet<u32> =
            procs.iter().filter(|p| apps.contains_key(&p.exe)).map(|p| p.pid).collect();
        match win::unity_on(&sink, &managed) {
            Ok(0) => {}
            Ok(n) => println!("set {n} app session(s) on the sink to 100%"),
            Err(e) => println!("can't set sink session volumes: {e}"),
        }

        // mics: rows "mic:<capture id>" whose device is present
        let mic_keys: Vec<String> =
            state.lock().unwrap().inputs.keys().filter(|k| k.starts_with("mic:")).cloned().collect();
        mics.retain(|k, _| captures.contains_key(&k[4..]));
        for key in mic_keys {
            let id = key[4..].to_string();
            if !captures.contains_key(&id) || mics.get(&key).is_some_and(Worker::alive) {
                continue;
            }
            println!("capturing mic {}", captures[&id]);
            let (k, c) = (key.clone(), conns.clone());
            mics.insert(key, Worker::spawn(captures[&id].clone(), move |stop| audio::capture(k, Source::Mic(id), c, stop)));
        }

        // outputs: present -> rendering (restart if it died); unplugged -> stopped, settings kept
        let out_ids: Vec<String> = state.lock().unwrap().outputs.keys().cloned().collect();
        outs.retain(|id, _| {
            let present = renders.contains_key(id);
            if !present {
                println!("output disconnected: {id}");
            }
            present
        });
        for id in out_ids {
            if !renders.contains_key(&id) || outs.get(&id).is_some_and(Worker::alive) {
                continue;
            }
            println!("output {}", renders[&id]);
            let (o, s, c) = (id.clone(), state.clone(), conns.clone());
            outs.insert(id.clone(), Worker::spawn(renders[&id].clone(), move |stop| audio::render(o, s, c, stop)));
        }

        // one queue per running (input, output) pair
        let inputs: Vec<String> = apps.keys().map(|e| app_key(e)).chain(mics.keys().cloned()).collect();
        let wanted: HashSet<(String, String)> =
            inputs.iter().flat_map(|i| outs.keys().map(move |o| (i.clone(), o.clone()))).collect();
        {
            let mut c = conns.lock().unwrap();
            c.retain(|k, _| wanted.contains(k));
            for k in wanted {
                c.entry(k).or_default();
            }
        }

        if dirty {
            state.lock().unwrap().save(&path)?;
            seen = mtime(&path);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // give apps their normal output back, or they stay silent while the engine is off
    let procs = win::processes()?;
    for exe in apps.keys() {
        let _ = win::route(&procs, exe, None);
    }
    state.lock().unwrap().save(&path)?;
    println!("stopped, {} app(s) returned to their normal output", apps.len());
    DONE.store(true, Ordering::Relaxed);
    Ok(())
}

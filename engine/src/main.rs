//! Audiomeda engine: headless matrix mixer. Owns the state, captures every app that
//! plays sound (routing its original output to the silent sink) and mixes it to each output.
// no console window when started at login; logs still reach a terminal or redirect (see main)
#![windows_subsystem = "windows"]

mod audio;
mod pipe;
mod state;
mod tray;
mod win;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use audio::{Conns, Source, Worker};
use audiomeda_protocol::{PIPE, Request, Response, Snapshot, State, app_key};
use wasapi::{Direction, initialize_mta};
use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE, SetConsoleCtrlHandler};
use windows_core::BOOL;
use win::Res;

/// Set by Ctrl+C or the tray's Quit; the main loop then un-routes apps and exits.
pub(crate) static STOP: AtomicBool = AtomicBool::new(false);
static DONE: AtomicBool = AtomicBool::new(false);
/// Set when the state changed and should be saved.
static DIRTY: AtomicBool = AtomicBool::new(false);

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
    // started from a terminal with no redirect: print there (a GUI-subsystem exe has no console)
    unsafe {
        if GetStdHandle(STD_OUTPUT_HANDLE).map_or(true, |h| h.is_invalid() || h.0.is_null()) {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn first_run_state() -> Res<State> {
    let renders = win::devices(Direction::Render)?;
    // VB-Cable C by default (Discord stuttered into cable D during screen share); changeable in
    // the mixer. Matched with its driver name: other virtual devices may be renamed "CABLE-C Input".
    let sink = renders
        .iter()
        .find(|(_, name)| name.starts_with("CABLE-C Input (VB-Audio"))
        .map(|(id, _)| id.clone())
        .ok_or("no \"CABLE-C Input (VB-Audio ...)\" device for the sink; install VB-Cable C+D or edit state.json")?;
    let (primary, name) = win::default_render()?;
    if primary == sink {
        return Err("the Windows default output is the sink cable; pick your headphones as default first".into());
    }
    Ok(State::new(sink, primary, name))
}

fn run() -> Res<()> {
    initialize_mta().ok()?;
    unsafe { SetConsoleCtrlHandler(Some(on_ctrl), true)? };

    let path = state::path();
    let state = match state::load(&path)? {
        Some(s) => s,
        None => {
            let s = first_run_state()?;
            state::save(&s, &path)?;
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
    let state = Arc::new(Mutex::new(state));
    // what clients see besides the state; refreshed every tick
    let live: Arc<Mutex<Snapshot>> = Default::default();
    let handler: tray::Handler = {
        let (state, live) = (state.clone(), live.clone());
        // never hold the state and live locks at the same time
        Arc::new(move |req| match req {
            Request::Snapshot => {
                let s = state.lock().unwrap().clone();
                Response::Snapshot(Snapshot { state: s, ..live.lock().unwrap().clone() })
            }
            req => {
                let (renders, captures) = {
                    let l = live.lock().unwrap();
                    (l.renders.clone(), l.captures.clone())
                };
                match state.lock().unwrap().apply(&req, &renders, &captures) {
                    Ok(()) => {
                        DIRTY.store(true, Ordering::Relaxed);
                        Response::Ok
                    }
                    Err(message) => Response::Error { message },
                }
            }
        })
    };
    let h = handler.clone();
    pipe::serve(PIPE, move |req| h(req))?;
    println!("listening on {PIPE}");
    let tray = tray::spawn(handler, state.clone());
    let conns: Conns = Default::default();
    let own_pid = std::process::id();

    let mut apps: HashMap<String, (u32, Worker)> = HashMap::new(); // exe -> (root pid, capture)
    let mut mics: HashMap<String, Worker> = HashMap::new(); // input key -> capture
    let mut outs: HashMap<String, Worker> = HashMap::new(); // device id -> render
    let mut routed_to = String::new(); // the sink running apps were last moved to

    // shortcut: polls every 500 ms instead of device/session notifications; switch if the
    // half-second delay before a new app is heard ever matters.
    while !STOP.load(Ordering::Relaxed) {
        // read every tick, so a sink change from the mixer applies live
        let sink = state.lock().unwrap().sink.clone();
        let procs = win::processes()?;
        let renders = win::devices(Direction::Render)?;
        let captures = win::devices(Direction::Capture)?;

        // apps playing anywhere but the sink are new or escaped (e.g. restarted): route + manage
        let by_pid = win::exe_by_pid(&procs);
        let playing: HashSet<String> = win::playing_pids(&[sink.as_str()])?
            .into_iter()
            .filter(|&pid| pid != 0 && pid != own_pid)
            .filter_map(|pid| by_pid.get(&pid).map(|e| e.to_string()))
            .collect();
        // apps making sound on any device, the sink included (e.g. started while the sink is
        // Windows' default, so they never played anywhere else)
        let sounding: HashSet<String> = win::playing_pids(&[])?
            .into_iter()
            .filter(|&pid| pid != 0 && pid != own_pid)
            .filter_map(|pid| by_pid.get(&pid).map(|e| e.to_string()))
            .collect();
        for exe in &sounding {
            if state.lock().unwrap().ensure_app(exe) {
                DIRTY.store(true, Ordering::Relaxed);
            }
        }
        for exe in &playing {
            // re-sent every tick until the app actually moves (some only move when playback restarts)
            let r = win::route(&procs, exe, Some(&sink));
            if !apps.contains_key(exe) {
                match r {
                    Ok(()) => println!("routed {exe} to sink"),
                    Err(e) => println!("can't route {exe}, it will be heard twice: {e}"),
                }
            }
        }

        // sink changed: move every managed app, silent ones too (playing ones are caught above)
        if sink != routed_to {
            if !routed_to.is_empty() {
                println!("sink changed, moving {} app(s)", apps.len());
            }
            for exe in apps.keys() {
                let _ = win::route(&procs, exe, Some(&sink));
            }
            routed_to = sink.clone();
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

        // shown in the mixer = has a window, or is making sound (e.g. Discord in the tray during a call)
        let with_windows = win::exes_with_windows(&by_pid);
        let playing_apps: Vec<String> = apps.keys().filter(|e| sounding.contains(*e)).map(|e| app_key(e)).collect();
        let open_apps: Vec<String> = apps
            .keys()
            .filter(|e| sounding.contains(*e) || with_windows.contains(*e))
            .map(|e| app_key(e))
            .collect();
        // remember exe paths so pinned apps keep their icon after closing (and across restarts)
        for (exe, (pid, _)) in &apps {
            if let Some(path) = win::exe_path(*pid) {
                let mut s = state.lock().unwrap();
                if s.app_paths.get(&app_key(exe)) != Some(&path) {
                    s.app_paths.insert(app_key(exe), path);
                    DIRTY.store(true, Ordering::Relaxed);
                }
            }
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
        mics.retain(|k, _| captures.contains_key(&k[4..]) && mic_keys.contains(k));
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
            present && out_ids.contains(id)
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

        *live.lock().unwrap() = Snapshot {
            state: State::default(), // filled in per request
            active_outputs: outs.keys().cloned().collect(),
            active_inputs: inputs,
            renders,
            captures,
            open_apps,
            playing_apps,
        };
        if DIRTY.swap(false, Ordering::Relaxed) {
            let s = state.lock().unwrap().clone();
            state::save(&s, &path)?;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // give apps their normal output back, or they stay silent while the engine is off
    let procs = win::processes()?;
    for exe in apps.keys() {
        let _ = win::route(&procs, exe, None);
    }
    let s = state.lock().unwrap().clone();
    state::save(&s, &path)?;
    println!("stopped, {} app(s) returned to their normal output", apps.len());
    let _ = tray.join(); // removes the tray icon
    DONE.store(true, Ordering::Relaxed);
    Ok(())
}

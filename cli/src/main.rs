//! `audiomeda`: tiny command-line remote for the engine (until the UI exists).
//! Inputs, outputs and devices match by id or by part of their name.

use audiomeda_protocol::{Client, Request, Response, Snapshot, mic_key};

const USAGE: &str = "usage:
  audiomeda status
  audiomeda devices
  audiomeda vol <input|output> <percent>        set an input's or output's master volume
  audiomeda vol <input> <output> <percent>      set one cell (also turns it on)
  audiomeda mute <input|output> on|off
  audiomeda default <output> <percent>          route for newly seen apps
  audiomeda reset <input>                       back to the default route
  audiomeda output add|remove <device>
  audiomeda mic add|remove <device>
  audiomeda mics toggle                         mute/unmute all mics
  audiomeda preset save|load|delete <name>
  audiomeda hotkey <combo> toggle_mics|preset:<name>|none
                                                e.g. hotkey ctrl+alt+m toggle_mics";

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if args.is_empty() {
        return Err(USAGE.into());
    }
    let mut client = Client::connect().map_err(|e| format!("engine isn't running ({e}); start audiomeda-engine.exe"))?;
    let snap = match call(&mut client, &Request::Snapshot)? {
        Response::Snapshot(s) => s,
        other => return Err(format!("unexpected reply: {other:?}")),
    };
    let req = match args.as_slice() {
        ["status"] => return Ok(status(&snap)),
        ["devices"] => return Ok(devices(&snap)),
        ["vol", target, pct] => {
            let volume = Some(percent(pct)?);
            match input_or_output(&snap, target)? {
                Ok(key) => Request::SetInput { key, volume, mute: None },
                Err(id) => Request::SetOutput { id, volume, mute: None },
            }
        }
        ["vol", inp, out, pct] => Request::SetCell {
            input: input(&snap, inp)?,
            output: output(&snap, out)?,
            volume: Some(percent(pct)?),
            on: Some(true),
        },
        ["mute", target, flag @ ("on" | "off")] => {
            let mute = Some(*flag == "on");
            match input_or_output(&snap, target)? {
                Ok(key) => Request::SetInput { key, volume: None, mute },
                Err(id) => Request::SetOutput { id, volume: None, mute },
            }
        }
        ["default", out, pct] => {
            Request::SetDefaultCell { output: output(&snap, out)?, volume: Some(percent(pct)?), on: Some(true) }
        }
        ["reset", inp] => Request::ResetInput { key: input(&snap, inp)? },
        ["output", "add", dev] => Request::AddOutput { id: pick("output device", dev, snap.renders.iter().map(|(k, v)| (k.as_str(), v.clone())))? },
        ["output", "remove", out] => Request::RemoveOutput { id: output(&snap, out)? },
        ["mic", "add", dev] => Request::AddMic { id: pick("input device", dev, snap.captures.iter().map(|(k, v)| (k.as_str(), v.clone())))? },
        ["mic", "remove", inp] => {
            let key = input(&snap, inp)?;
            Request::RemoveMic { id: key.strip_prefix("mic:").ok_or("that input isn't a mic")?.to_string() }
        }
        ["mics", "toggle"] => Request::ToggleMics,
        ["preset", "save", name] => Request::SavePreset { name: name.to_string() },
        ["preset", "load", name] => Request::LoadPreset { name: name.to_string() },
        ["preset", "delete", name] => Request::DeletePreset { name: name.to_string() },
        ["hotkey", combo, action] => Request::SetHotkey {
            combo: combo.to_string(),
            action: (*action != "none").then(|| action.to_string()),
        },
        _ => return Err(USAGE.into()),
    };
    match call(&mut client, &req)? {
        Response::Ok => Ok(println!("ok")),
        other => Err(format!("unexpected reply: {other:?}")),
    }
}

fn call(client: &mut Client, req: &Request) -> Result<Response, String> {
    match client.call(req).map_err(|e| format!("talking to the engine failed: {e}"))? {
        Response::Error { message } => Err(message),
        r => Ok(r),
    }
}

fn percent(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim_end_matches('%').parse().map_err(|_| format!("not a percentage: {s}"))?;
    Ok(v / 100.0)
}

fn input_name(snap: &Snapshot, key: &str) -> String {
    match key.strip_prefix("mic:") {
        Some(id) => format!("mic {}", snap.captures.get(id).map_or(id, String::as_str)),
        None => key.trim_start_matches("app:").to_string(),
    }
}

fn input(snap: &Snapshot, q: &str) -> Result<String, String> {
    pick("input", q, snap.state.inputs.keys().map(|k| (k.as_str(), input_name(snap, k))))
}

fn output(snap: &Snapshot, q: &str) -> Result<String, String> {
    pick("output", q, snap.state.outputs.iter().map(|(k, c)| (k.as_str(), c.name.clone())))
}

/// For commands that accept either: try inputs, then outputs.
fn input_or_output(snap: &Snapshot, q: &str) -> Result<Result<String, String>, String> {
    match input(snap, q) {
        Ok(key) => Ok(Ok(key)),
        Err(_) => output(snap, q).map(Err).map_err(|_| format!("no input or output matches {q:?}")),
    }
}

/// Exact id, else the one candidate whose name contains `q` (case-insensitive).
fn pick<'a>(what: &str, q: &str, candidates: impl Iterator<Item = (&'a str, String)>) -> Result<String, String> {
    let ql = q.to_lowercase();
    let mut hits = Vec::new();
    for (key, name) in candidates {
        if key == q {
            return Ok(key.to_string());
        }
        if name.to_lowercase().contains(&ql) {
            hits.push((key, name));
        }
    }
    match hits.as_slice() {
        [(key, _)] => Ok(key.to_string()),
        [] => Err(format!("no {what} matches {q:?}")),
        _ => Err(format!(
            "{q:?} matches several {what}s: {}",
            hits.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

fn status(snap: &Snapshot) {
    let s = &snap.state;
    let flag = |muted: bool, active: bool| match (muted, active) {
        (true, _) => "muted",
        (false, true) => "live",
        (false, false) => "offline",
    };
    println!("OUTPUTS");
    for (id, c) in &s.outputs {
        println!("  {:<44} {:>5}  {}", c.name, pct(c.volume), flag(c.mute, snap.active_outputs.contains(id)));
    }
    println!("INPUTS");
    for (key, row) in &s.inputs {
        let cells: Vec<String> = row
            .cells
            .iter()
            .filter(|(_, c)| c.on)
            .map(|(id, c)| format!("{} {}", s.outputs.get(id).map_or(id.as_str(), |o| &o.name), pct(c.volume)))
            .collect();
        let to = if cells.is_empty() { "(nowhere)".into() } else { cells.join(", ") };
        let active = snap.active_inputs.contains(key);
        println!("  {:<44} {:>5}  {:<7} -> {to}", input_name(snap, key), pct(row.volume), flag(row.mute, active));
    }
    let def: Vec<String> = s
        .default_route
        .iter()
        .filter(|(_, c)| c.on)
        .map(|(id, c)| format!("{} {}", s.outputs.get(id).map_or(id.as_str(), |o| &o.name), pct(c.volume)))
        .collect();
    println!("NEW APPS -> {}", def.join(", "));
    let presets: Vec<&str> = s.presets.keys().map(String::as_str).collect();
    println!("PRESETS: {}", if presets.is_empty() { "(none)".into() } else { presets.join(", ") });
    for (combo, action) in &s.hotkeys {
        println!("HOTKEY {combo} -> {action}");
    }
}

fn devices(snap: &Snapshot) {
    println!("OUTPUT DEVICES (for `output add`)");
    for (id, name) in &snap.renders {
        let tag = if *id == snap.state.sink { " (sink)" } else if snap.state.outputs.contains_key(id) { " (in use)" } else { "" };
        println!("  {name}{tag}");
    }
    println!("INPUT DEVICES (for `mic add`)");
    for (id, name) in &snap.captures {
        let tag = if snap.state.inputs.contains_key(&mic_key(id)) { " (in use)" } else { "" };
        println!("  {name}{tag}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_matches_id_or_unique_name() {
        let c = || [("id1", "Speakers (HyperX)".to_string()), ("id2", "VG271 P".to_string()), ("id3", "PL2788H".to_string())].into_iter();
        assert_eq!(pick("x", "hyperx", c()).unwrap(), "id1");
        assert_eq!(pick("x", "id3", c()).unwrap(), "id3");
        assert!(pick("x", "nope", c()).is_err());
        assert!(pick("x", "P", c()).is_err(), "ambiguous");
        assert_eq!(percent("30%").unwrap(), 0.3);
        assert!(percent("loud").is_err());
    }
}

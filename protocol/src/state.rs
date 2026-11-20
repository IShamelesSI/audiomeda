//! Mixer state, the engine's single source of truth.
//! Volumes are linear 0.0..=1.0; the UI maps fader position through a dB curve.

use crate::{Devices, Request};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Input keys are "app:<exe, lowercase>" or "mic:<capture device id>".
/// Output keys are render device ids (physical outputs and virtual-mic cables).
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct State {
    /// Render device that managed apps are routed to so they aren't heard twice.
    pub sink: String,
    pub outputs: BTreeMap<String, Column>,
    pub inputs: BTreeMap<String, Row>,
    /// Cells a newly seen app starts with.
    pub default_route: BTreeMap<String, Cell>,
    #[serde(default)]
    pub presets: BTreeMap<String, Preset>,
    /// Canonical combo ("ctrl+alt+m") -> action ("toggle_mics" or "preset:<name>").
    #[serde(default)]
    pub hotkeys: BTreeMap<String, String>,
}

/// A saved mix. Loading it replaces outputs and the default route, and overwrites the
/// rows it has; apps it doesn't know keep their current row.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Preset {
    pub outputs: BTreeMap<String, Column>,
    pub inputs: BTreeMap<String, Row>,
    pub default_route: BTreeMap<String, Cell>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub volume: f32,
    pub mute: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Row {
    pub volume: f32,
    pub mute: bool,
    pub cells: BTreeMap<String, Cell>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Cell {
    pub volume: f32,
    pub on: bool,
}

pub fn app_key(exe: &str) -> String {
    format!("app:{}", exe.to_lowercase())
}

pub fn mic_key(device_id: &str) -> String {
    format!("mic:{device_id}")
}

fn check(v: Option<f32>) -> Result<Option<f32>, String> {
    match v {
        Some(x) if !(0.0..=1.0).contains(&x) => Err(format!("volume {x} is outside 0..1")),
        v => Ok(v),
    }
}

fn unknown(what: &str, key: &str) -> String {
    format!("unknown {what}: {key}")
}

/// Parses a global hotkey like "Ctrl+Alt+M" into (Win32 MOD_* flags, virtual-key code,
/// canonical text). Needs a modifier so it can't swallow normal typing.
pub fn parse_hotkey(s: &str) -> Result<(u32, u32, String), String> {
    const MODS: [(&str, u32); 4] = [("ctrl", 2), ("alt", 1), ("shift", 4), ("win", 8)];
    let (mut mods, mut key) = (0, None);
    for part in s.to_lowercase().split('+').map(str::trim) {
        let part = if part == "control" { "ctrl" } else { part };
        if let Some((_, m)) = MODS.iter().find(|(n, _)| *n == part) {
            mods |= m;
        } else if key.is_some() {
            return Err(format!("{s:?} has more than one key"));
        } else {
            let vk = match part.as_bytes() {
                [c @ (b'a'..=b'z' | b'0'..=b'9')] => c.to_ascii_uppercase() as u32,
                _ if part == "space" => 0x20,
                _ => match part.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
                    Some(n @ 1..=24) => 0x6F + n,
                    _ => return Err(format!("unknown key {part:?} (use a-z, 0-9, f1-f24 or space)")),
                },
            };
            key = Some((vk, part.to_string()));
        }
    }
    let (vk, name) = key.ok_or_else(|| format!("{s:?} has no key"))?;
    if mods == 0 {
        return Err(format!("{s:?} needs ctrl, alt, shift or win"));
    }
    let mut parts: Vec<&str> = MODS.iter().filter(|(_, m)| mods & m != 0).map(|(n, _)| *n).collect();
    parts.push(&name);
    Ok((mods, vk, parts.join("+")))
}

impl State {
    /// First run: primary output at 100%, nothing else.
    pub fn new(sink: String, primary_id: String, primary_name: String) -> Self {
        let mut s = State { sink, ..Default::default() };
        s.outputs.insert(primary_id.clone(), Column { name: primary_name, volume: 1.0, mute: false });
        s.default_route.insert(primary_id, Cell { volume: 1.0, on: true });
        s
    }

    /// Final gain of one input on one output: row × cell × column, 0 if anything is muted/off/missing.
    pub fn gain(&self, input: &str, output: &str) -> f32 {
        let (Some(row), Some(col)) = (self.inputs.get(input), self.outputs.get(output)) else {
            return 0.0;
        };
        match row.cells.get(output) {
            Some(c) if c.on && !row.mute && !col.mute => row.volume * c.volume * col.volume,
            _ => 0.0,
        }
    }

    fn mics_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.inputs.iter_mut().filter(|(k, _)| k.starts_with("mic:")).map(|(_, r)| r)
    }

    /// True if there are mics and all of them are muted.
    pub fn mics_muted(&self) -> bool {
        let mut mics = self.inputs.iter().filter(|(k, _)| k.starts_with("mic:")).peekable();
        mics.peek().is_some() && mics.all(|(_, r)| r.mute)
    }

    fn new_app_row(&self) -> Row {
        Row { volume: 1.0, mute: false, cells: self.default_route.clone() }
    }

    /// Adds a row for a newly seen app from the default route. Returns true if it was new.
    pub fn ensure_app(&mut self, exe: &str) -> bool {
        let key = app_key(exe);
        if self.inputs.contains_key(&key) {
            return false;
        }
        let row = self.new_app_row();
        self.inputs.insert(key, row);
        true
    }

    /// Applies a change from a client. `renders`/`captures` are the devices present now,
    /// used to validate devices being added. Nothing changes if it returns an error.
    pub fn apply(&mut self, req: &Request, renders: &Devices, captures: &Devices) -> Result<(), String> {
        match req {
            Request::Snapshot => {}
            Request::SetInput { key, volume, mute } => {
                let volume = check(*volume)?;
                let row = self.inputs.get_mut(key).ok_or_else(|| unknown("input", key))?;
                row.volume = volume.unwrap_or(row.volume);
                row.mute = mute.unwrap_or(row.mute);
            }
            Request::SetOutput { id, volume, mute } => {
                let volume = check(*volume)?;
                let col = self.outputs.get_mut(id).ok_or_else(|| unknown("output", id))?;
                col.volume = volume.unwrap_or(col.volume);
                col.mute = mute.unwrap_or(col.mute);
            }
            Request::SetCell { input, output, volume, on } => {
                let volume = check(*volume)?;
                if !self.outputs.contains_key(output) {
                    return Err(unknown("output", output));
                }
                let row = self.inputs.get_mut(input).ok_or_else(|| unknown("input", input))?;
                let cell = row.cells.entry(output.clone()).or_insert(Cell { volume: 0.0, on: true });
                cell.volume = volume.unwrap_or(cell.volume);
                cell.on = on.unwrap_or(cell.on);
            }
            Request::SetDefaultCell { output, volume, on } => {
                let volume = check(*volume)?;
                if !self.outputs.contains_key(output) {
                    return Err(unknown("output", output));
                }
                let cell = self.default_route.entry(output.clone()).or_insert(Cell { volume: 0.0, on: true });
                cell.volume = volume.unwrap_or(cell.volume);
                cell.on = on.unwrap_or(cell.on);
            }
            Request::ResetInput { key } => {
                let fresh = if key.starts_with("app:") {
                    self.new_app_row()
                } else {
                    Row { volume: 1.0, mute: false, cells: BTreeMap::new() }
                };
                *self.inputs.get_mut(key).ok_or_else(|| unknown("input", key))? = fresh;
            }
            Request::AddOutput { id } => {
                if *id == self.sink {
                    return Err("that device is the sink; it can't also be an output".into());
                }
                let name = renders.get(id).ok_or_else(|| unknown("output device", id))?;
                self.outputs
                    .entry(id.clone())
                    .or_insert(Column { name: name.clone(), volume: 1.0, mute: false });
            }
            Request::RemoveOutput { id } => {
                self.outputs.remove(id).ok_or_else(|| unknown("output", id))?;
                for row in self.inputs.values_mut() {
                    row.cells.remove(id);
                }
                self.default_route.remove(id);
            }
            Request::AddMic { id } => {
                if !captures.contains_key(id) {
                    return Err(unknown("input device", id));
                }
                // new mics are routed nowhere, so you never suddenly hear yourself
                self.inputs
                    .entry(mic_key(id))
                    .or_insert(Row { volume: 1.0, mute: false, cells: BTreeMap::new() });
            }
            Request::RemoveMic { id } => {
                self.inputs.remove(&mic_key(id)).ok_or_else(|| unknown("mic", id))?;
            }
            Request::ToggleMics => {
                if self.mics_mut().next().is_none() {
                    return Err("there are no mics".into());
                }
                let mute = !self.mics_muted();
                self.mics_mut().for_each(|r| r.mute = mute);
            }
            Request::SavePreset { name } => {
                let name = name.trim();
                if name.is_empty() {
                    return Err("preset name is empty".into());
                }
                let p = Preset {
                    outputs: self.outputs.clone(),
                    inputs: self.inputs.clone(),
                    default_route: self.default_route.clone(),
                };
                self.presets.insert(name.to_string(), p);
            }
            Request::LoadPreset { name } => {
                let p = self.presets.get(name).cloned().ok_or_else(|| unknown("preset", name))?;
                self.outputs = p.outputs;
                self.default_route = p.default_route;
                self.inputs.extend(p.inputs);
            }
            Request::DeletePreset { name } => {
                self.presets.remove(name).ok_or_else(|| unknown("preset", name))?;
                let action = format!("preset:{name}");
                self.hotkeys.retain(|_, a| *a != action);
            }
            Request::SetHotkey { combo, action } => {
                let (_, _, combo) = parse_hotkey(combo)?;
                match action.as_deref() {
                    None => {
                        self.hotkeys.remove(&combo).ok_or_else(|| unknown("hotkey", &combo))?;
                    }
                    Some(a) => {
                        let ok = a == "toggle_mics"
                            || a.strip_prefix("preset:").is_some_and(|p| self.presets.contains_key(p));
                        if !ok {
                            return Err(format!("unknown action {a:?} (use toggle_mics or preset:<saved preset>)"));
                        }
                        self.hotkeys.insert(combo, a.to_string());
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> State {
        let mut s = State::new("sink".into(), "hp".into(), "Headphones".into());
        s.outputs.insert("vr".into(), Column { name: "VR".into(), volume: 0.5, mute: false });
        s
    }

    fn devs(ids: &[&str]) -> Devices {
        ids.iter().map(|i| (i.to_string(), format!("{i} name"))).collect()
    }

    #[test]
    fn new_app_gets_default_route_and_keeps_edits() {
        let mut s = sample();
        assert!(s.ensure_app("MSEdge.exe"));
        assert_eq!(s.gain("app:msedge.exe", "hp"), 1.0);
        assert_eq!(s.gain("app:msedge.exe", "vr"), 0.0); // not in default route

        let row = s.inputs.get_mut("app:msedge.exe").unwrap();
        row.cells.get_mut("hp").unwrap().volume = 0.3;
        row.cells.insert("vr".into(), Cell { volume: 0.6, on: true });
        assert!(!s.ensure_app("msedge.exe")); // seen again: edits kept
        assert!((s.gain("app:msedge.exe", "hp") - 0.3).abs() < 1e-6);
        assert!((s.gain("app:msedge.exe", "vr") - 0.3).abs() < 1e-6); // 0.6 cell × 0.5 column
    }

    #[test]
    fn mute_and_off_silence() {
        let mut s = sample();
        s.ensure_app("a.exe");
        s.inputs.get_mut("app:a.exe").unwrap().mute = true;
        assert_eq!(s.gain("app:a.exe", "hp"), 0.0);
        s.inputs.get_mut("app:a.exe").unwrap().mute = false;
        s.outputs.get_mut("hp").unwrap().mute = true;
        assert_eq!(s.gain("app:a.exe", "hp"), 0.0);
        s.outputs.get_mut("hp").unwrap().mute = false;
        s.inputs.get_mut("app:a.exe").unwrap().cells.get_mut("hp").unwrap().on = false;
        assert_eq!(s.gain("app:a.exe", "hp"), 0.0);
        assert_eq!(s.gain("app:missing.exe", "hp"), 0.0);
    }

    #[test]
    fn apply_sets_and_validates() {
        let mut s = sample();
        s.ensure_app("a.exe");
        let none = Devices::new();
        let set_cell = |v| Request::SetCell { input: "app:a.exe".into(), output: "vr".into(), volume: Some(v), on: None };
        s.apply(&set_cell(0.6), &none, &none).unwrap();
        assert!((s.gain("app:a.exe", "vr") - 0.3).abs() < 1e-6);

        let before = s.clone();
        assert!(s.apply(&set_cell(1.5), &none, &none).is_err());
        assert!(s.apply(&set_cell(f32::NAN), &none, &none).is_err());
        let bad_out = Request::SetCell { input: "app:a.exe".into(), output: "nope".into(), volume: None, on: None };
        assert!(s.apply(&bad_out, &none, &none).is_err());
        assert_eq!(s, before, "failed requests change nothing");

        s.apply(&Request::SetInput { key: "app:a.exe".into(), volume: None, mute: Some(true) }, &none, &none).unwrap();
        assert_eq!(s.inputs["app:a.exe"].volume, 1.0); // None left unchanged
        s.apply(&Request::ResetInput { key: "app:a.exe".into() }, &none, &none).unwrap();
        assert_eq!(s.inputs["app:a.exe"], s.new_app_row());
    }

    #[test]
    fn hotkey_parsing() {
        assert_eq!(parse_hotkey("Alt+Ctrl+M").unwrap(), (3, 0x4D, "ctrl+alt+m".into()));
        assert_eq!(parse_hotkey("win+shift+F12").unwrap(), (12, 0x7B, "shift+win+f12".into()));
        assert_eq!(parse_hotkey("ctrl+1").unwrap().1, 0x31);
        assert!(parse_hotkey("m").is_err(), "needs a modifier");
        assert!(parse_hotkey("ctrl+a+b").is_err());
        assert!(parse_hotkey("ctrl+f25").is_err());
        assert!(parse_hotkey("ctrl+").is_err());
    }

    #[test]
    fn presets_mics_and_hotkeys() {
        let mut s = sample();
        let none = Devices::new();
        s.ensure_app("a.exe");
        s.apply(&Request::AddMic { id: "m".into() }, &none, &devs(&["m"])).unwrap();
        s.apply(&Request::SavePreset { name: "quiet".into() }, &none, &none).unwrap();

        // change things, then load: rows in the preset come back, outputs replaced, new apps kept
        s.inputs.get_mut("app:a.exe").unwrap().volume = 0.2;
        s.outputs.remove("vr");
        s.ensure_app("new.exe");
        s.apply(&Request::LoadPreset { name: "quiet".into() }, &none, &none).unwrap();
        assert_eq!(s.inputs["app:a.exe"].volume, 1.0);
        assert!(s.outputs.contains_key("vr"));
        assert!(s.inputs.contains_key("app:new.exe"));

        assert!(!s.mics_muted());
        s.apply(&Request::ToggleMics, &none, &none).unwrap();
        assert!(s.mics_muted());
        s.apply(&Request::ToggleMics, &none, &none).unwrap();
        assert!(!s.mics_muted());

        let hk = |c: &str, a: Option<&str>| Request::SetHotkey { combo: c.into(), action: a.map(Into::into) };
        s.apply(&hk("Ctrl+Alt+1", Some("preset:quiet")), &none, &none).unwrap();
        assert_eq!(s.hotkeys["ctrl+alt+1"], "preset:quiet");
        assert!(s.apply(&hk("ctrl+alt+2", Some("preset:nope")), &none, &none).is_err());
        assert!(s.apply(&hk("ctrl+alt+2", Some("explode")), &none, &none).is_err());
        s.apply(&Request::DeletePreset { name: "quiet".into() }, &none, &none).unwrap();
        assert!(s.hotkeys.is_empty(), "hotkeys for a deleted preset go too");

        let mut empty = sample();
        assert!(empty.apply(&Request::ToggleMics, &none, &none).is_err());
    }

    #[test]
    fn add_and_remove_devices() {
        let mut s = sample();
        s.ensure_app("a.exe");
        let renders = devs(&["tv", "sink"]);
        let captures = devs(&["mic1"]);
        s.apply(&Request::AddOutput { id: "tv".into() }, &renders, &captures).unwrap();
        assert_eq!(s.outputs["tv"].name, "tv name");
        assert!(s.apply(&Request::AddOutput { id: "sink".into() }, &renders, &captures).is_err());
        assert!(s.apply(&Request::AddOutput { id: "gone".into() }, &renders, &captures).is_err());

        s.apply(&Request::AddMic { id: "mic1".into() }, &renders, &captures).unwrap();
        assert_eq!(s.gain("mic:mic1", "hp"), 0.0, "new mics are routed nowhere");
        s.apply(&Request::RemoveMic { id: "mic1".into() }, &renders, &captures).unwrap();
        assert!(!s.inputs.contains_key("mic:mic1"));

        s.apply(&Request::RemoveOutput { id: "hp".into() }, &renders, &captures).unwrap();
        assert!(!s.inputs["app:a.exe"].cells.contains_key("hp"));
        assert!(!s.default_route.contains_key("hp"));
    }
}

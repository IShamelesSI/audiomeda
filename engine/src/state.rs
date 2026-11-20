//! Mixer state: the single source of truth, saved as JSON.
//! Volumes are linear 0.0..=1.0; the UI maps fader position through a dB curve.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

    /// Adds a row for a newly seen app from the default route. Returns true if it was new.
    pub fn ensure_app(&mut self, exe: &str) -> bool {
        let key = app_key(exe);
        if self.inputs.contains_key(&key) {
            return false;
        }
        let cells = self.default_route.clone();
        self.inputs.insert(key, Row { volume: 1.0, mute: false, cells });
        true
    }

    pub fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| ".".into());
        base.join("Audiomeda").join("state.json")
    }

    pub fn load(path: &Path) -> std::io::Result<Option<State>> {
        match std::fs::read_to_string(path) {
            // hand-edited files may start with a UTF-8 BOM (PowerShell 5.1 writes one)
            Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}'))
                .map(Some)
                .map_err(|e| std::io::Error::other(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Write to a temp file, then rename, so a crash mid-write can't corrupt the saved state.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)
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
    fn save_load_roundtrip() {
        let mut s = sample();
        s.ensure_app("a.exe");
        let path = std::env::temp_dir().join(format!("audiomeda-test-{}.json", std::process::id()));
        s.save(&path).unwrap();
        assert_eq!(State::load(&path).unwrap(), Some(s));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(State::load(&path).unwrap(), None);
    }
}

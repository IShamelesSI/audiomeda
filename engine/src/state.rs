//! Saving and loading the mixer state (types and logic live in `audiomeda-protocol`).

use audiomeda_protocol::State;
use std::path::{Path, PathBuf};

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
pub fn save(state: &State, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_roundtrip() {
        let mut s = State::new("sink".into(), "hp".into(), "Headphones".into());
        s.ensure_app("a.exe");
        let path = std::env::temp_dir().join(format!("audiomeda-test-{}.json", std::process::id()));
        save(&s, &path).unwrap();
        assert_eq!(load(&path).unwrap(), Some(s));
        std::fs::write(&path, "\u{feff}{\"sink\":\"x\",\"outputs\":{},\"inputs\":{},\"default_route\":{}}").unwrap();
        assert_eq!(load(&path).unwrap().unwrap().sink, "x", "BOM is tolerated");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(load(&path).unwrap(), None);
    }
}

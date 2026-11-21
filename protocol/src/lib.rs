//! What the engine and its clients (CLI, later the UI) say to each other over the
//! named pipe `\\.\pipe\audiomeda`: one JSON object per line, each request gets one
//! response. Also holds the mixer state they share, and its pure logic.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};

mod state;
pub use state::*;

pub const PIPE: &str = r"\\.\pipe\audiomeda";

/// Device id -> friendly name.
pub type Devices = BTreeMap<String, String>;

/// Volumes are 0.0..=1.0. `None` fields are left unchanged.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Snapshot,
    SetInput { key: String, volume: Option<f32>, mute: Option<bool> },
    SetOutput { id: String, volume: Option<f32>, mute: Option<bool> },
    SetCell { input: String, output: String, volume: Option<f32>, on: Option<bool> },
    SetDefaultCell { output: String, volume: Option<f32>, on: Option<bool> },
    /// Back to how a newly seen app starts.
    ResetInput { key: String },
    AddOutput { id: String },
    RemoveOutput { id: String },
    /// The silent cable managed apps play into (the engine captures them from there).
    SetSink { id: String },
    AddMic { id: String },
    RemoveMic { id: String },
    /// Mutes all mics, or unmutes them all if they're all muted already.
    ToggleMics,
    SavePreset { name: String },
    LoadPreset { name: String },
    DeletePreset { name: String },
    /// `action` is "toggle_mics" or "preset:<name>"; `None` removes the hotkey.
    SetHotkey { combo: String, action: Option<String> },
    /// Replaces the custom name/color/pin of an input or output. An empty label removes it.
    SetLabel { key: String, label: Label },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Snapshot(Snapshot),
    Ok,
    Error { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub state: State,
    /// Devices present right now.
    pub renders: Devices,
    pub captures: Devices,
    /// Inputs being captured and outputs being rendered right now.
    pub active_inputs: Vec<String>,
    pub active_outputs: Vec<String>,
    /// App inputs that have a visible window or are playing sound: what the mixer shows.
    pub open_apps: Vec<String>,
    /// App inputs making sound right now.
    pub playing_apps: Vec<String>,
}

/// Blocking client: one request, one response.
pub struct Client {
    reader: BufReader<File>,
    writer: File,
}

impl Client {
    pub fn connect() -> io::Result<Self> {
        Self::connect_to(PIPE)
    }

    pub fn connect_to(pipe: &str) -> io::Result<Self> {
        const ERROR_PIPE_BUSY: i32 = 231;
        let mut tries = 0;
        let f = loop {
            match OpenOptions::new().read(true).write(true).open(pipe) {
                // the engine is between accepting one client and listening for the next
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && tries < 100 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                r => break r?,
            }
        };
        Ok(Client { writer: f.try_clone()?, reader: BufReader::new(f) })
    }

    pub fn call(&mut self, req: &Request) -> io::Result<Response> {
        let mut line = serde_json::to_string(req)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;
        let mut resp = String::new();
        if self.reader.read_line(&mut resp)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "engine closed the connection"));
        }
        serde_json::from_str(&resp).map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format() {
        let r: Request = serde_json::from_str(r#"{"op":"set_input","key":"app:x.exe","volume":0.5}"#).unwrap();
        assert_eq!(r, Request::SetInput { key: "app:x.exe".into(), volume: Some(0.5), mute: None });
        assert_eq!(serde_json::to_string(&Response::Ok).unwrap(), r#"{"type":"ok"}"#);
        let snap = Response::Snapshot(Snapshot::default());
        assert_eq!(serde_json::from_str::<Response>(&serde_json::to_string(&snap).unwrap()).unwrap(), snap);
    }
}

//! Thin bridge: the web UI calls `call(request)`, which goes to the engine over the named
//! pipe. The UI holds no state of its own.

mod icon;

use audiomeda_protocol::{Client, Request, Response};
use std::sync::Mutex;

#[derive(Default)]
struct Pipe(Mutex<Option<Client>>);

/// Runs on a worker thread (`async`), so a slow engine never freezes the window.
#[tauri::command(async)]
fn call(req: Request, pipe: tauri::State<Pipe>) -> Result<Response, String> {
    let mut conn = pipe.0.lock().unwrap();
    // a kept connection may be dead if the engine restarted: reconnect once
    for attempt in 0..2 {
        if conn.is_none() {
            *conn = Some(Client::connect().map_err(|e| format!("The engine isn't running ({e})"))?);
        }
        match conn.as_mut().unwrap().call(&req) {
            Ok(resp) => return Ok(resp),
            Err(e) if attempt == 1 => return Err(format!("Lost the connection to the engine ({e})")),
            Err(_) => *conn = None,
        }
    }
    unreachable!()
}

/// The exe's icon as a PNG data URL (None if it has no usable icon).
#[tauri::command(async)]
fn app_icon(path: String) -> Option<String> {
    icon::png_data_url(&path)
}

pub fn run() {
    tauri::Builder::default()
        .manage(Pipe::default())
        .invoke_handler(tauri::generate_handler![call, app_icon])
        .run(tauri::generate_context!())
        .expect("error while running the Audiomeda UI");
}

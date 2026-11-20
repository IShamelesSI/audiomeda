//! Named-pipe server: one thread per connected client, one JSON line in, one out.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::Arc;

use audiomeda_protocol::{Request, Response};
use windows::Win32::Foundation::{CloseHandle, ERROR_BROKEN_PIPE, ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAGS_AND_ATTRIBUTES, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows_core::HSTRING;

use crate::win::Res;

const MAX_LINE: u64 = 1 << 20;

struct Pipe(HANDLE);
// the handle is only ever used by the one thread that owns this value
unsafe impl Send for Pipe {}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut n = 0;
        match unsafe { ReadFile(self.0, Some(buf), Some(&mut n), None) } {
            Ok(()) => Ok(n as usize),
            Err(e) if e.code() == ERROR_BROKEN_PIPE.to_hresult() => Ok(0), // client hung up
            Err(e) => Err(io::Error::other(e)),
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut n = 0;
        unsafe { WriteFile(self.0, Some(buf), Some(&mut n), None) }.map_err(io::Error::other)?;
        Ok(n as usize)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe {
            let _ = DisconnectNamedPipe(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

fn create(name: &str, first: bool) -> io::Result<Pipe> {
    let first_flag = if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { FILE_FLAGS_AND_ATTRIBUTES(0) };
    let h = unsafe {
        CreateNamedPipeW(
            &HSTRING::from(name),
            PIPE_ACCESS_DUPLEX | first_flag,
            // local clients only: the engine never listens on the network
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            None,
        )
    };
    if h.is_invalid() { Err(io::Error::last_os_error()) } else { Ok(Pipe(h)) }
}

/// Claims the pipe now (fails if another engine already owns it), then serves clients on a
/// background thread for the rest of the process.
pub fn serve(name: &str, handler: impl Fn(Request) -> Response + Send + Sync + 'static) -> Res<()> {
    let first = create(name, true).map_err(|e| format!("can't open {name}, is another engine running? ({e})"))?;
    let handler = Arc::new(handler);
    let name = name.to_string();
    std::thread::spawn(move || {
        let mut next = Some(first);
        loop {
            let pipe = match next.take().map_or_else(|| create(&name, false), Ok) {
                Ok(p) => p,
                Err(e) => {
                    println!("pipe: can't create instance: {e}");
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    continue;
                }
            };
            if let Err(e) = unsafe { ConnectNamedPipe(pipe.0, None) } {
                // a client that connected between create and connect is fine
                if e.code() != ERROR_PIPE_CONNECTED.to_hresult() {
                    continue;
                }
            }
            let h = handler.clone();
            std::thread::spawn(move || client(pipe, &*h));
        }
    });
    Ok(())
}

fn client(pipe: Pipe, handler: &dyn Fn(Request) -> Response) {
    let mut reader = BufReader::new(pipe);
    let mut line = String::new();
    loop {
        line.clear();
        match (&mut reader).take(MAX_LINE).read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handler(req),
            Err(e) => Response::Error { message: format!("bad request: {e}") },
        };
        let mut out = serde_json::to_string(&resp).expect("responses always serialize");
        out.push('\n');
        if reader.get_mut().write_all(out.as_bytes()).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiomeda_protocol::Client;

    #[test]
    fn roundtrip_and_single_owner() {
        let name = format!(r"\\.\pipe\audiomeda-test-{}", std::process::id());
        serve(&name, |req| match req {
            Request::Snapshot => Response::Ok,
            other => Response::Error { message: format!("{other:?}") },
        })
        .unwrap();
        assert!(serve(&name, |_| Response::Ok).is_err(), "a second engine can't take the pipe");

        let mut a = Client::connect_to(&name).unwrap();
        let mut b = Client::connect_to(&name).unwrap(); // several clients at once
        assert_eq!(a.call(&Request::Snapshot).unwrap(), Response::Ok);
        assert!(matches!(b.call(&Request::RemoveMic { id: "x".into() }).unwrap(), Response::Error { .. }));
        assert_eq!(a.call(&Request::Snapshot).unwrap(), Response::Ok, "connection stays open");
    }
}

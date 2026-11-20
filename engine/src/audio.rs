//! Audio threads: one capture thread per input, one render thread per output.
//! They meet in `Conns`: one sample queue per (input, output) pair.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use wasapi::{AudioClient, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};

use audiomeda_protocol::State;
use crate::win::Res;

pub const RATE: usize = 48_000;
const CH: usize = 2;
const MS: usize = RATE * CH / 1000; // samples per millisecond
// Spike showed apps deliver in bursts of up to ~40 ms; only trim queues well above that.
// shortcut: trimming drops audio (click); upgrade to resampling drift correction if clicks show up.
const QUEUE_MAX_MS: usize = 80;
const QUEUE_TARGET_MS: usize = 20;

/// Interleaved stereo f32 @ 48 kHz per (input key, output id).
/// shortcut: one lock for all queues; fine for a handful of inputs/outputs, split per output if it ever contends.
pub type Conns = Arc<Mutex<HashMap<(String, String), VecDeque<f32>>>>;

pub enum Source {
    App(u32), // root pid, whole process tree is captured
    Mic(String),
}

/// A running thread; stops itself (within ~200 ms) when dropped.
pub struct Worker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

impl Worker {
    pub fn spawn(name: String, f: impl FnOnce(&AtomicBool) -> Res<()> + Send + 'static) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let handle = std::thread::spawn(move || {
            let r = initialize_mta().ok().map_err(Into::into).and_then(|_| f(&s));
            if let Err(e) = r {
                println!("[{name}] stopped: {e}");
            }
        });
        Worker { stop, handle }
    }

    pub fn alive(&self) -> bool {
        !self.handle.is_finished()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn format() -> WaveFormat {
    WaveFormat::new(32, 32, SampleType::Float, RATE, CH, None)
}

pub fn capture(key: String, src: Source, conns: Conns, stop: &AtomicBool) -> Res<()> {
    let (mut client, period) = match src {
        Source::App(pid) => (AudioClient::new_application_loopback_client(pid, true)?, 0),
        Source::Mic(id) => {
            let c = DeviceEnumerator::new()?.get_device(&id)?.get_iaudioclient()?;
            let (period, _) = c.get_device_period()?;
            (c, period)
        }
    };
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: period };
    client.initialize_client(&format(), Direction::Capture, mode, None)?;
    let event = client.set_get_eventhandle()?;
    let cc = client.get_audiocaptureclient()?;
    client.start_stream()?;
    let mut buf = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        // app loopback gets no events while the app is silent, so time out and re-check `stop`
        let _ = event.wait_for_event(200);
        while let Some(frames) = cc.get_next_packet_size()?.filter(|&n| n > 0) {
            buf.resize(frames as usize * CH * 4, 0);
            let (got, _) = cc.read_from_device(&mut buf)?;
            let samples = buf[..got as usize * CH * 4]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            let mut conns = conns.lock().unwrap();
            for (_, q) in conns.iter_mut().filter(|((i, _), _)| *i == key) {
                q.extend(samples.clone());
                if q.len() > QUEUE_MAX_MS * MS {
                    let extra = q.len() - QUEUE_TARGET_MS * MS;
                    q.drain(..extra - extra % CH);
                }
            }
        }
    }
    Ok(())
}

pub fn render(out: String, state: Arc<Mutex<State>>, conns: Conns, stop: &AtomicBool) -> Res<()> {
    let mut client = DeviceEnumerator::new()?.get_device(&out)?.get_iaudioclient()?;
    let (period, _) = client.get_device_period()?;
    // shortcut: plain shared mode gives ~22 ms device buffer; IAudioClient3 low-latency
    // streams would cut that for virtual-mic outputs (needs raw windows calls, no autoconvert).
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: period };
    client.initialize_client(&format(), Direction::Render, mode, None)?;
    let event = client.set_get_eventhandle()?;
    let rc = client.get_audiorenderclient()?;
    rc.write_to_device(&vec![0u8; client.get_available_space_in_frames()? as usize * CH * 4], None)?;
    client.start_stream()?;

    while !stop.load(Ordering::Relaxed) {
        if event.wait_for_event(200).is_err() {
            continue;
        }
        let want = client.get_available_space_in_frames()? as usize * CH;
        // never hold the state and conns locks at the same time
        let gains: HashMap<String, f32> = {
            let s = state.lock().unwrap();
            s.inputs.keys().map(|k| (k.clone(), s.gain(k, &out))).collect()
        };
        let mut mix = vec![0f32; want];
        for ((input, _), q) in conns.lock().unwrap().iter_mut().filter(|((_, o), _)| *o == out) {
            let g = gains.get(input).copied().unwrap_or(0.0);
            let take = want.min(q.len());
            for (m, s) in mix.iter_mut().zip(q.drain(..take)) {
                *m += s * g;
            }
        }
        let bytes: Vec<u8> = mix.iter().flat_map(|s| s.to_le_bytes()).collect();
        rc.write_to_device(&bytes, None)?;
    }
    Ok(())
}

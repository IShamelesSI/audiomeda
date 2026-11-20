# Audiomeda — design decisions

Software audio mixer for Windows 11. Personal project, nothing redistributed.

## Core idea
Per-app audio, each app mixed to every output at its own volume
(e.g. browser 30% on headphones, 60% on VR headset), plus mic + app mixes
sent to virtual mics (e.g. mic + browser → Discord for singing).

## Audio plumbing
- **Windows 11 only** (per-process loopback capture needs build 20348+).
- **Per-app capture**: managed apps are routed (undocumented
  `IAudioPolicyConfig`, as EarTrumpet does) to a silent "Audiomeda" sink cable;
  the engine captures each app separately via process loopback
  (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`, include process tree).
  The sink exists only so the original isn't heard twice. It's required:
  muting the app instead also silences the capture (spike step 2).
- **Virtual devices**: VB-Cable + A+B + C+D (5 cables): 1 sink (cable D),
  3 virtual mics (Discord, game voice chat, OBS), 1 spare. Engine picks devices by ID; which
  cable plays which role is config. No own driver (test-signing breaks
  anti-cheat games and needs Secure Boot off).
- **WASAPI shared mode**, ~10 ms buffers, target < 20 ms mic → output.
- Internal mix: 48 kHz, 32-bit float; resample per device.

## Mixer model
- Matrix: rows = inputs (apps, mics), columns = outputs (physical devices,
  virtual mics). Gain = row master × cell × column master. Mute on each.
  dB internally, % in UI.
- New apps: auto-routed by an editable default route (initially: primary
  output 100%, rest 0%). Per-exe overrides remembered; "reset to default".
- Unplugged outputs keep their column + settings (greyed out), resume on
  reconnect. Tracked by device ID.

## Processes
- **Engine**: headless background process in the user session, starts at
  login, native tray icon (open UI / presets / mute mic / quit), global
  hotkeys, toast on preset change. Single source of truth; state in
  `%APPDATA%\Audiomeda\` as JSON, debounced write on change.
- **UI**: Tauri + Svelte, opened only when needed; talks to engine over named
  pipe `\\.\pipe\audiomeda`. Meter stream ~30 fps only while UI is open.
  Holds no state.

## UI
- Skinnable without touching function: state + engine API in Svelte stores
  and a typed API layer; thin components styled only by design tokens (CSS
  variables, incl. animation durations/easings); skins = rewrite `.svelte`
  views only. Dev playground page showing every component in every state.
- First draft: dark studio (charcoal, glowing meters) + translucency;
  per-app cards main view, toggleable matrix view. Meters on `<canvas>`.

## Scope
- v1: per-app × per-output volume, mute, meters, mic inputs, 3 virtual mics,
  presets, tray, hotkeys, mic gain + noise gate.
- Later: EQ/compressor, recorder, reverb.
- Skipped: VBAN, macros.

## Spike results (2026-11-20, Win11 build 26200)
The throwaway spike CLI is deleted; its working code lives on in `engine/`.
1. **Per-app routing: pass.** `IAudioPolicyConfig` must get the HSTRING
   *handle*, not its text pointer. For browsers, only the main/audio processes
   accept it (sandboxed children return E_INVALIDARG): try every PID and
   ignore those failures. Moving to a new device may need the app to restart
   playback; moving back took effect immediately.
2. **Capture while the app plays into the sink: pass (2A).** Muting the app's
   session also silences its loopback capture (2B fail), so **the sink cable
   stays**.
3. **One app → two outputs at 30% / 60%: pass.** 6 min with no audible drift.
   Occasional 30–40 ms drops hit both outputs at once, which means bursty app
   delivery, not clock drift. Engine: make rings tolerant of bursts (bigger
   high-water mark) before adding resampling drift correction.
4. **Mic + app → CABLE-A → Discord: pass.** ~22 ms render buffer + ~10 ms
   mic ≈ 30 ms, over the 20 ms target. Engine: request smaller shared-mode
   buffers (IAudioClient3 / minimum period) on virtual-mic outputs.

## Engine core (built 2026-11-20)
`engine/`, run `target\release\audiomeda-engine.exe`. State lives in
`%APPDATA%\Audiomeda\state.json`.
- Every 500 ms the engine checks for any app with an active session on a
  non-sink device. It routes that app to the sink, adds it to the state with
  the default route (first time only), and captures its process tree. Every
  known app that is running gets captured. Captures restart if the app
  restarts.
- One render thread per configured output that is present. Unplugging stops
  it (logged as "output disconnected") and reconnecting restarts it, with
  settings kept. Tested by unplugging the HyperX.
- Mic rows are state keys `mic:<capture device id>`. Volumes are stored
  linear 0–1 (the UI will map faders through a dB curve).
- **Finding:** loopback capture is *post* the app's Windows session volume,
  which Windows remembers per device (Opera was at 14% on the sink). The
  engine forces managed apps to 100% and unmuted on the sink, so only mixer
  volumes count.
- Ctrl+C returns apps to their normal output. After a crash or kill, run
  `audiomeda-engine reset`, or apps stay silent (routed to the sink).
- Changes go through the named pipe (below). `state.json` is read only at
  startup, so hand edits need the engine stopped (e.g. to change the sink).
- Known shortcuts (marked `shortcut:` in code): polling instead of
  notifications, one lock for all queues, drift fixed by trimming queues,
  ~22 ms shared-mode output buffer (IAudioClient3 low latency still to do).

## Named pipe API (built 2026-11-20)
- `\\.\pipe\audiomeda`, newline-delimited JSON, one response per request.
  Message types are in `protocol/` (`Request`, `Response`, `Snapshot`).
- `snapshot` returns the state, the devices present, and which inputs and
  outputs are live. Everything else is a change (set input, output, cell or
  default cell, reset input, add/remove output or mic). Changes are
  validated (volume 0–1, known ids, sink can't be an output); a failed
  request changes nothing. The engine saves after each change.
- Security: local clients only (`PIPE_REJECT_REMOTE_CLIENTS`). The engine
  claims the first pipe instance, so a second engine refuses to start.
- Clients retry briefly when every pipe instance is busy (race between
  accepting one client and listening for the next).
- No push notifications or meter stream yet; the UI step adds those (clients
  poll `snapshot` until then).

## Repo layout
Rust workspace:
- `protocol/`: messages + mixer state and its pure logic (`apply`, `gain`),
  shared by engine, CLI and later the UI. No Windows code.
- `engine/`: `main.rs` manager loop, `audio.rs` capture/render threads,
  `pipe.rs` named-pipe server, `state.rs` load/save, `win.rs` Windows plumbing.
- `cli/`: `audiomeda` command-line remote.
- Later: `ui/` (Tauri + Svelte).

## Build order
1. ✅ Spike: all 4 risky questions answered (results above).
2. ✅ Engine core: headless matrix mixer, JSON state, hot-plug.
3. ✅ Named pipe API + tiny CLI client.
4. Tray, presets, hotkeys (usable daily without UI).
5. Tauri + Svelte UI: stores/API, tokens, playground, cards view.
6. Matrix view, mic noise gate.
7. Later features.

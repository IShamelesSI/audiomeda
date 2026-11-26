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
- **Virtual devices**: VB-Cable + A+B + C+D (5 cables): 1 sink (cable C by
  default, switchable live with `set_sink`; Discord stuttered into cable D
  during screen share but not into C),
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
  settings kept. Tested by unplugging a USB headset.
- Mic rows are state keys `mic:<capture device id>`. Volumes are stored
  linear 0–1 (the UI will map faders through a dB curve).
- **Finding:** loopback capture is *post* the app's Windows session volume,
  which Windows remembers per device (Opera was at 14% on the sink). The
  engine forces managed apps to 100% and unmuted on the sink, so only mixer
  volumes count.
- Ctrl+C returns apps to their normal output. After a crash or kill, run
  `audiomeda-engine reset`, or apps stay silent (routed to the sink).
- Changes go through the named pipe (below). `state.json` is read only at
  startup, so hand edits need the engine stopped.
- The sink is re-read every tick: after `set_sink`, apps still on the old
  sink count as playing elsewhere and are moved to the new one.
- Apps already playing into the sink (e.g. started while the sink was
  Windows' default) are picked up too, so they're never silent.
- When an app runs as sibling process trees under a launcher whose children
  are all that app (League: six LeagueClientUxRender under LeagueClientUx),
  the launcher's tree is captured; otherwise the app's topmost process.
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

## Tray, presets, hotkeys (built 2026-11-20)
- The engine is a windowless app (`windows_subsystem = "windows"`). Its
  logs still print when run from a terminal (it attaches to the parent
  console) or with output redirected.
- `tray.rs`: one thread owns a hidden window that receives tray clicks,
  hotkeys and a 250 ms timer. The tray menu has load preset, mute mics (a
  checkmark shows the state), Start with Windows, and Quit. Quit stops
  gracefully, returning apps to their normal output.
- Tray actions and hotkeys go through the same request handler as the
  pipe, so the validation and saving are shared.
- **Presets** (`state.presets`): a snapshot of levels (output masters, input
  rows, default route). Loading one never adds or removes outputs, apps or
  mics: it only sets volume, mute and sends for devices that exist now and
  that the preset knows. Everything else keeps its current settings.
- **Hotkeys** (`state.hotkeys`: combo → `toggle_mics` | `preset:<name>`).
  A modifier is required. They're re-registered when the list changes; a
  combo taken by another app shows a toast. Deleting a preset removes its
  hotkeys.
- Toasts (tray balloon notifications) appear on hotkey actions and errors.
- Start with Windows = per-user `HKCU\...\Run` value "Audiomeda" pointing
  at the exe. Off by default; toggled from the tray.
- Icons: `assets/source/` holds the originals (app image, pink tray image).
  `assets/make_icons.py` builds every icon from them: `app.ico` / `tray.ico`
  (cleans faint alpha noise, crops square, multi-size) with full-size PNGs,
  the UI window icons in `ui/src-tauri/icons/` and the header logo
  `ui/static/logo.png`.
  `engine/build.rs` embeds both (app = exe icon, tray = resource 2) and a
  per-monitor DPI-aware manifest so the tray icon stays sharp.
- Left-click on the tray icon (or the bold "Open mixer" item in the menu)
  opens the mixer: it brings the existing "Audiomeda" window forward, or
  starts `audiomeda-ui.exe` from next to the engine exe.
- Not in yet: log file for when it starts at login.

## Mixer UI (2026-11-20)
- `ui/`: Tauri 2 + SvelteKit (SPA, static adapter) + Svelte 5. Dark window
  with Mica showing through (`transparent` + `windowEffects: mica`).
- `ui/src-tauri/src/lib.rs`: `call(request)` goes to the engine over the
  pipe (reconnects once if the engine restarted); `app_icon(path)` returns an
  exe's icon as a PNG data URL (`icon.rs`, `SHDefExtractIconW`). The mix lives
  only in the engine.
- `ui/src/lib/api/engine.svelte.ts` is the only code that talks to the
  engine: it polls `snapshot` every 300 ms (shortcut until the engine pushes
  changes) and sends changes. While a fader is dragged, only the newest value
  per target is sent. `types.ts` mirrors the protocol by hand.
- Skinnable: every color, size and timing is in
  `ui/src/lib/theme/tokens.css` (Svelte transitions read their durations
  from it too; reduced motion zeroes them). Components in
  `ui/src/lib/components/` are thin and style only with tokens. Re-skin =
  tokens and/or `.svelte` views; `lib/api` stays untouched.
- Faders show position %; gain = position² (audio taper), same curve in the
  CLI via `protocol::gain_from_position`.
- Main view: two columns with a draggable divider (25–75%, double-click
  resets, arrow keys move it). Left: **Outputs** (physical) and **Apps**.
  Right: **Input devices** (mics) and **Virtual outputs** (cables, matched by
  name: cable / vb-audio / virtual / voicemeeter ...). Each column fills the
  window height and scrolls on its own; all card grids share one minimum
  width (`--card-min`), so they reflow at the same widths.
- Input cards: master, mute, pin (apps), rename/recolor (mics), and one send
  per output (on/off pill + level) in the same order as the output cards,
  physical first, then virtual.
- Apps come and go: the engine reports `open_apps` (a visible, unowned,
  titled top-level window) and `playing_apps` (an active session). An app
  shows while open or playing, or always when pinned.
- Labels (`state.labels`, key = app/mic key or output id): custom name,
  color, pinned. Shown as "Custom (Windows name)" on the device's own card,
  just "Custom" everywhere else. Saved by the engine, so they survive a
  reboot. The engine also records each app's exe path (`state.app_paths`)
  so closed pinned apps keep their real icon.
- Cards drag to reorder within their section (grab any spot that isn't a
  control; the column auto-scrolls near its edges). Order and divider width
  are window preferences in `localStorage` (WebView2 keeps it across
  reboots), not engine state.
- Small buttons are outlined and glow on hover; their icon pops on click;
  mute turns red, pin turns violet while engaged.
- Presets as chips in the header. A toast shows engine errors; a banner shows
  when the engine is unreachable. A `svelte:boundary` in `+layout.svelte`
  shows the error and a "Try again" button instead of a blank window if a
  render crashes (Svelte 5 forbids writing `$state` during render; that
  caused one blank window).
- `/playground` (linked in dev builds): every component in every state with
  fake data.
- Build: dev `cd ui && npm run tauri dev`; release `npx tauri build --no-bundle`
  puts `target/release/audiomeda-ui.exe` next to the engine.
- Not in yet: level meters (needs engine metering + a stream), matrix view,
  engine push instead of polling, single-instance UI.

## Repo layout
Rust workspace:
- `protocol/`: messages + mixer state and its pure logic (`apply`, `gain`),
  shared by engine, CLI and later the UI. No Windows code.
- `engine/`: `main.rs` manager loop, `audio.rs` capture/render threads,
  `pipe.rs` named-pipe server, `tray.rs` tray/hotkeys/toasts/autostart,
  `state.rs` load/save, `win.rs` Windows plumbing.
- `cli/`: `audiomeda` command-line remote.
- `assets/`: app and tray icons, their sources, and the script that builds them.
- `ui/`: mixer window (Tauri + Svelte); Rust bridge in `ui/src-tauri/`.
  - `src/lib/api/`: the only code that talks to the engine (`engine.svelte.ts`),
    protocol types, display-name helpers.
  - `src/lib/components/`: cards, faders, buttons, menus.
  - `src/lib/actions/`: Svelte actions (drag to reorder).
  - `src/lib/theme/`: design tokens, global CSS, motion helper.
  - `src/routes/`: the mixer page and `/playground`.

## Build order
1. ✅ Spike: all 4 risky questions answered (results above).
2. ✅ Engine core: headless matrix mixer, JSON state, hot-plug.
3. ✅ Named pipe API + tiny CLI client.
4. ✅ Tray, presets, hotkeys (usable daily without UI).
5. ✅ Tauri + Svelte UI: API layer, tokens, playground, cards view, labels,
   pins, real icons, two-column layout, drag to reorder.
6. Matrix view, mic noise gate.
7. Later features.

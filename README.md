# Audiomeda

A per-app audio mixer for Windows 11. Each app can play on every output at its
own volume, and mic + app mixes go out through virtual mics. Design and
decisions are in [DESIGN.md](DESIGN.md).

## Requirements
- Windows 11
- Rust (stable, MSVC)
- VB-Cable, A+B and C+D installed. "CABLE-D Input" is the silent sink.

## Run
```
cargo build --release -p audiomeda-engine -p audiomeda-cli
cd ui && npm install && npx tauri build --no-bundle && cd ..
target\release\audiomeda-engine.exe
```
Left-click the tray icon to open the mixer window. For UI work with instant
reload, use `cd ui && npm run tauri dev`; the `/playground` page (linked in
dev builds) shows every component with fake data.
The first run creates `%APPDATA%\Audiomeda\state.json`, with your current
default output as the only output.

## Control it
While the engine runs, `target\release\audiomeda.exe` changes the mix. Names
match by any part, e.g. `opera` or `hyperx`.
```
audiomeda status                      see the whole matrix
audiomeda devices                     list devices you can add
audiomeda output add VG271            add an output
audiomeda vol opera hyperx 30         Opera at 30% on the headphones
audiomeda vol opera 50                Opera's master volume
audiomeda mute discord on
audiomeda mic add "Microphone (HyperX"
audiomeda output add "CABLE-A Input"  virtual mic for Discord
audiomeda vol "mic Microphone" "CABLE-A Input" 100
audiomeda preset save Gaming          snapshot the current mix
audiomeda hotkey ctrl+alt+1 preset:Gaming
audiomeda hotkey ctrl+alt+m toggle_mics
```
Run `audiomeda` with no arguments for the full list.

The engine has no window, only a tray icon (look under the ^ next to the
clock). Its menu loads presets, mutes mics, turns on **Start with Windows**,
and **Quit**s, which gives apps their normal output back. If the engine was
killed instead, run `target\release\audiomeda-engine.exe reset`, or apps stay
silent.

## The mixer window
- Left column: **Outputs** and **Apps**; right column: **Input devices** and
  **Virtual outputs**. Drag the divider to resize; double-click resets it.
- Apps appear while they have a window or play sound. Pin one to keep it
  when closed.
- The pencil on outputs and mics renames and recolors them.
- Drag cards by an empty spot to reorder them within their section.
- Loading a preset changes levels only; it never adds or removes devices.

## Change the look
Colors, sizes and animation timings are all in
`ui/src/lib/theme/tokens.css`. Components live in `ui/src/lib/components/`.
To change the icons, replace the images in `assets/source/` and run
`python assets/make_icons.py` (needs Pillow).

## Project layout
```
protocol/   messages and mixer state logic, shared by everything
engine/     the headless mixer: audio, named pipe, tray, hotkeys
cli/        audiomeda.exe, a command-line remote
ui/         the mixer window (Tauri + Svelte)
assets/     icon sources and the script that builds every icon
```
Details and decisions: [DESIGN.md](DESIGN.md).

## Test
```
cargo test
cd ui && npm run check
```

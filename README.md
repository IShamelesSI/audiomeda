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
cargo build --release
target\release\audiomeda-engine.exe
```
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
```
Run `audiomeda` with no arguments for the full list.

Stop with **Ctrl+C**, which gives apps their normal output back. If the engine
was killed instead, run `target\release\audiomeda-engine.exe reset`, or apps
stay silent.

## Test
```
cargo test
```

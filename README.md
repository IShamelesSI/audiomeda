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
default output as the only output. Edit that file while the engine runs
and the changes apply within half a second.

Stop with **Ctrl+C**, which gives apps their normal output back. If the engine
was killed instead, run `target\release\audiomeda-engine.exe reset`, or apps
stay silent.

## Test
```
cargo test
```

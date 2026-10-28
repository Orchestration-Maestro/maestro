# DOOM overlay

Play DOOM in a terminal overlay with `/doom-overlay [WAD path]`. Q
pauses the retained game and closes the view. Reopening resumes the same game;
changing the WAD replaces the engine before opening the new selection.

## WAD files

An explicit path overrides discovery. Without one, the example checks the
component's directory, `./doom1.wad`, `./DOOM1.WAD`, `~/doom1.wad` and
`~/.doom/doom1.wad`, in that order. If no local file exists, it downloads the
shareware WAD beside the component. Custom WAD files are not required to match
the shareware checksum.

The download uses the [PureDOOM mirror pinned at commit
355cfbd16fac119718879239336ee2ea408886bd](https://raw.githubusercontent.com/Daivuk/PureDOOM/355cfbd16fac119718879239336ee2ea408886bd/doom1.wad).
It must match the official DOOM shareware v1.9 file:

- Size: 4,196,020 bytes.
- SHA-256: `1d7d43be501e67d927e415e0b8f3e29c3bf33075e859721816f652a526cac771`.
- MD5 identification: `f0cefca49926d00903cf57551d901abe`.

The example rejects a checksum mismatch before creating its cache file. A
network, body or filesystem failure produces a notification without opening a
game. No game data ships with Maestro.

## Build the opt-in engine

The player component alone has an unresolved engine import. Build the engine
separately, then compose it with the player. Maestro ships MIT glue only: no
engine source, engine binary or WAD. The external engine is GPL-2.0; its license
still applies to artifacts you build or distribute. Do not put its source or the
composed artifact in the shipped workspace or package.

Install Python 3.9 or newer, Git, wasi-sdk 34, wit-bindgen-cli 0.62.0 and wasm-tools 1.262.0.
Put `wit-bindgen` and `wasm-tools` on PATH and set `WASI_SDK_PATH` to the SDK root.
Install the component target with `rustup target add wasm32-wasip2`.
From the workspace root, using an empty external build directory:

```sh
export DOOM_BUILD="$HOME/.cache/maestro-doom"
mkdir -p "$DOOM_BUILD"
git clone https://github.com/ozkl/doomgeneric "$DOOM_BUILD/source"
git -C "$DOOM_BUILD/source" checkout --detach e257a09527062bfb986b6e5e0780839b6cbd5b71
python3 examples/extensions/doom-overlay/build_engine.py \
  --source "$DOOM_BUILD/source" --output "$DOOM_BUILD/engine.wasm"
CARGO_TARGET_DIR="$DOOM_BUILD/target" cargo build --locked --release \
  -p maestro-extensions-wasm --target wasm32-wasip2 --example doom_overlay
wasm-tools compose \
  "$DOOM_BUILD/target/wasm32-wasip2/release/examples/doom_overlay.wasm" \
  -d "$DOOM_BUILD/engine.wasm" -o "$DOOM_BUILD/doom_overlay.wasm"
```

Load the composed `doom_overlay.wasm` through the normal
[extension loading interface](../../docs/extensions/loading.md), not the
uncomposed player. Use an interactive component-enabled host with true-colour
half-block output, enhanced keyboard releases and native background waits.
Headless invocation reports that interactive mode is required.

## Controls and lifetime

Arrow keys turn and move; WASD moves and strafes. Shift+WASD runs, Space uses,
F fires, and 1–7 selects weapons. Enter and Escape operate game menus, Tab shows
the map. Q pauses the game and closes the overlay;
`/doom-overlay` resumes it. A changed WAD or failed engine reloads into a fresh
component and opens the requested selection once. Session resume does not
reopen a consumed request.

The engine has no window or audio backend. Its framebuffer is rendered at 35 Hz
using two sampled pixels per terminal cell. Closing the overlay cancels its wait
and releases held keys. Configuration, the copied IWAD and saves remain under
one private temporary root until reload or session shutdown removes it. Saves
are not retained between engine instances. Fatal engine messages return through
the component interface and become notifications instead of writing over the
host terminal.

## Controlled runs

```sh
cargo test --locked -p maestro-extensions-wasm --lib doom_overlay_
cargo test --locked -p maestro-extensions-wasmtime --test integration doom_
```

Ordinary tests use small MIT engines and synthetic WAD bytes. To qualify your
separately built engine and a lawful WAD without adding either to the tree:

```sh
MAESTRO_DOOM_ENGINE="$DOOM_BUILD/engine.wasm" \
MAESTRO_DOOM_WAD=/absolute/path/to/doom1.wad \
cargo test --locked -p maestro-extensions-wasmtime --test integration \
  real_engine_initializes_and_advances_frames -- --ignored
```

The opt-in check supplies only a private writable root, initializes the engine,
advances 210 ticks and asserts complete, opaque, changing RGBA frames.

# Terminal demonstrations

## Usage and setup

Run these examples in a disposable Unix terminal from the repository root:

```sh
cargo run -p maestro-tui-crossterm --example chat_simple
cargo run -p maestro-tui-crossterm --example image_test -- /path/to/image.png
cargo run -p maestro-tui-crossterm --example key_tester
cargo run -p maestro-tui-crossterm --example viewport_overwrite_repro
```

The examples compose the [native adapter](native-terminal.md) with the toolkit.
Their styles resolve the embedded brand pack through the shared [brand library](../brand.md).

## Chat

The focused [Editor](editor.md) offers `/delete` and `/clear` commands and native
file completion. Accepted ordinary messages become [Markdown](markdown.md);
a Thinking indicator precedes a simulated reply after one second. Delete removes the last
message; clear removes the message region, retaining the welcome and editor.
Ctrl+C exits, stopping the indicator and cancelling a pending reply.

## Image

The image example prints capabilities, the authored path and PNG dimensions.
An absent or empty argument selects `/tmp/test-image.png`. A read failure prints
the path and Cargo usage to stderr and exits with status 1 before terminal input
starts. Known dimensions select an [Image](images.md) capped at 60 cells;
otherwise a dimension diagnostic replaces it. Ctrl+C exits the display.
Capabilities and dimensions use Rust's Debug representation.

## Keys

The key example displays lowercase UTF-8 hex, Unicode scalar numbers and escaped
ESC, CR, LF, TAB and DEL characters. It retains the last 20 input events in
arrival order, clipping only rendered rows. Resize narrower and wider to inspect
stored input. Ctrl+C or SIGINT prints `Exiting...` after successful shutdown.
[Key decoding](keys.md) owns enhanced-key recognition.

## Viewport

The viewport example samples startup height, streams PRE rows, pauses for TOOL
rows and appends six POST rows. Use an 80-column terminal with 8–12 rows to
inspect [rendering](rendering.md) and scrollback while resizing. Height changes
do not change the sampled stream counts.

## Qualification and platforms

Qualify these programs manually in disposable real terminals: capture startup,
interaction, history, exit status and terminal modes before and after execution.
The shared executable root attempts to stop the original terminal after its
body returns; native adapter drop owns final descriptor restoration. An image
byte log is not evidence of decoded pixels, and terminal captures do not prove
physical input-method behavior. Non-Unix builds report Unsupported; Windows
execution remains tracked in [the native adapter issue](https://github.com/Orchestration-Maestro/maestro/issues/425).

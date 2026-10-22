# Copying text

Use `maestro_app::copy_to_clipboard(text).await` for desktop defaults, or
`copy_to_clipboard_with(text, &operations).await` with a `ClipboardOperations`
adapter. Both use the same policy; the adapter supplies native writes, process
launches and terminal output, and the policy reads the environment itself where it
needs it.

A successful native write on macOS or Windows finishes local copying. Otherwise,
macOS uses `pbcopy` and Windows uses `clip`. Other hosts try the Termux helper,
then `wl-copy` with a Wayland display, then `xclip` and `xsel` with an X11 display.
Native writes are skipped on Linux; hosts outside macOS, Linux and Windows have
no default native module. Helper writers receive unchanged UTF-8 input with output discarded.
Only synchronous helper writers have a five-second limit. The Wayland probe and
background writer have no deadline; the writer returns after acknowledged
startup, without waiting for exit.

Remote sessions identified by nonempty `SSH_CONNECTION`, `SSH_CLIENT` or
`MOSH_CONNECTION` also receive an OSC 52 terminal sequence. Local copying uses
that sequence only when native and helper routes fail. The padded base64 payload
must be at most 100,000 encoded bytes. An oversized payload does not undo a
successful copy; otherwise the error is `Failed to copy to clipboard`. Terminal
write errors propagate even after remote native success.

The routing, exact bytes, bounds and errors are exercised by
[the clipboard tests](../../crates/maestro-app/tests/clipboard_copy.rs).

## Reading images

Use [`utils::clipboard_image::read_clipboard_image`](../../crates/maestro-app/src/utils/clipboard_image/reader.rs)
for host defaults, or `read_clipboard_image_with` with a
`ClipboardImageOperations` adapter. The returned `ClipboardImage` owns its bytes.
An absent environment or platform option independently uses the process
environment and the platform the adapter reports; a supplied environment replaces
routing observations entirely, including when empty. Overrides do not change helper
environments or cached native admission. A nonempty Termux marker skips all effects.

Linux detects WSL from nonempty flags or the kernel version. Wayland and WSL try
`wl-paste`, then `xclip`; WSL additionally tries PowerShell through a temporary
PNG. Native reading is the final fallback outside Wayland, and the only route
on other platforms. Every attempted PowerShell route attempts temporary-file
cleanup, ignoring cleanup failures. Helper failures and missing/empty data return
no image; native probe/read failures propagate.

PNG, JPEG, WebP and GIF are preferred in that order. Selected raw labels retain
their parameters for helper arguments; returned labels use the normalized base
MIME type. Supported bytes pass through without validation. Other image formats
use the shared PNG converter without EXIF orientation or resizing; conversion
failure returns no image without restarting fallback.

Type listings and path conversion have a 1,000 ms deadline, image reads 3,000 ms,
and PowerShell 5,000 ms. Each command captures both descriptors with a combined
50 MiB bound and closed stdin. Native bytes and temporary-file reads are not
command-bounded. See [synchronous capture](../../crates/maestro-process/README.md#synchronous-capture)
and [image tests](../../crates/maestro-app/tests/clipboard_images.rs).

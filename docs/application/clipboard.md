# Copying text

Use `maestro_app::copy_to_clipboard(text).await` for desktop defaults, or
`copy_to_clipboard_with(text, &operations).await` with a `ClipboardOperations`
adapter. Both use the same policy; the adapter supplies native writes, environment
observations, process launches and terminal output.

A successful native write on macOS or Windows finishes local copying. Otherwise,
macOS uses `pbcopy` and Windows uses `clip`. Other hosts try the Termux helper,
then `wl-copy` with a Wayland display, then `xclip` and `xsel` with an X11 display.
Native writes are skipped on Linux. Writers receive unchanged UTF-8 input and a
five-second limit with output discarded; the Wayland probe has no deadline and
its writer returns after acknowledged startup, without waiting for exit.

Remote sessions identified by nonempty `SSH_CONNECTION`, `SSH_CLIENT` or
`MOSH_CONNECTION` also receive an OSC 52 terminal sequence. Local copying uses
that sequence only when native and helper routes fail. The padded base64 payload
must be at most 100,000 encoded bytes. An oversized payload does not undo a
successful copy; otherwise the error is `Failed to copy to clipboard`. Terminal
write errors propagate even after remote native success.

The routing, exact bytes, bounds and errors are exercised by
[the clipboard tests](../../crates/maestro-app/tests/clipboard_copy.rs).

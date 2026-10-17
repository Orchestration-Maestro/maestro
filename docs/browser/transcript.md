# Native copy controls

`maestro_web::components::copy_button::CopyButton` retains a supplied
`AttachmentHost`. Construct it with the current localization owner, set `text`,
`title` and `show_text`, then call `render` with the frame's UI and presentation.
The default text is empty, the title is localized `Copy` and success text is off.
An explicitly empty title stays empty.

Activation starts the native copy immediately, capturing the current original
text. Pending and rejected operations do not show a new success indication.
Success switches the copy glyph to a check and optionally shows localized
`Copied!` for 2000 ms after completion. A later success replaces that control's
restoring timer. A rejected timer registration clears feedback rather than
leaving a permanent success indication.

The control shares weak clipboard completion and timer ownership with message
links, which retain their separate 1500 ms policy. Dropping the control revokes
its timer and prevents pending work from delivering new UI feedback; it does not
claim to cancel native copying already started. Pointer activation requires both
primary endpoints in the enabled, visible slot on the admitted toolkit layer.
Keyboard activation remains available through the returned response.

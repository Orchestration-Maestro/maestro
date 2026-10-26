# Guest UI

A component reaches the host's user interface through the `ExtensionUIContext` its
context hands it: requests, the factories behind widgets, headers and footers, themes,
terminal input and dialogs. The [extension guide](guest.md#tool-renderers) describes
tool renderers and custom message components.

## Requests

`ExtensionUIContext` sends these synchronous requests to the installed host and
returns its rejection as a recoverable `ExtensionResult` error:

| Method | Request |
| --- | --- |
| `notify` | A literal message with an optional `NotificationType`. |
| `set_status` | Text for one status key, or that key's clearing. |
| `set_working_message` | Literal working text, or the host default. |
| `set_working_visible` | Show or hide the working loader row. |
| `set_working_indicator` | `WorkingIndicatorOptions` with independent optional `frames` and `interval_ms`, or the host default. |
| `set_hidden_thinking_label` | Literal collapsed-thinking text, or the host default. |
| `set_widget` | Fixed ordered lines for one key with optional `WidgetPlacement`, or that widget's removal. |
| `set_title` | A literal terminal title. |
| `paste_to_editor` | Literal text the host editor receives as a paste, through its paste handling. |
| `set_editor_text` | Literal replacement text for the host editor. |
| `get_editor_text` | Reads the host editor's current text, returned unchanged. |

The four option types are generated from the same interface file and re-exported at
the author root. `None` and a present empty value stay different requests. The text
methods forward what the caller supplied, so `Some("")` is an empty string the host
keeps, while `None` means what its method says: `set_status` clears that key's text,
`set_widget` removes the keyed widget, and `set_working_message`,
`set_working_indicator` and `set_hidden_thinking_label` ask the host to restore its
own default. `notify` without a severity leaves the notification's kind to the host.
Inside the option records, an absent `placement` or an absent `frames` or
`interval_ms` member stays absent, so the host applies its own default. Literal text
crosses unchanged, including whitespace, Unicode and styling bytes. Indicator frames
are supplied verbatim and `interval_ms` crosses as `f64` without serialization,
clamping or a guest timer. An empty frame list hides the indicator while `None`
restores the animated default; both the animation and the placement defaults stay
with the host.

A request borrows its text and lines only for the synchronous call; the host owns
anything it retains. Clones of the facade share the acquired resource, so a handle
kept past its callback keeps reading that resource, and a second acquisition stays
independent.

`set_widget_factory` shows the component a `ComponentFactory` builds under one key. The
factory runs synchronously with the lent `TUI`, the live `Theme` and a callback emitter
and returns an `ExtensionComponent`: the component and an optional disposal. The host
runs the disposal of the widget a key already shows before it runs the new factory, and
both finish before the setter returns. `set_widget` with lines or `None` disposes a
factory widget under the same key too. The factory's captures stay usable while the
host shows the component and are released after the host removes it. A factory error
is the setter's error.

`set_footer` replaces the footer with the component a `FooterFactory` builds from the
terminal, the theme and the `FooterData` the host lends, or restores the host's footer
with `None`; the host disposes the custom footer it replaces first. `FooterData` reads
the current branch, extension statuses in insertion order and provider count on every
call, so a retained footer sees later changes. `on_branch_change` keeps its
`BranchChangeListener` until the returned subscription is unsubscribed; dropping the
subscription keeps it.

`set_header` replaces the header with the component a factory builds, or restores the
host's header with `None`. The host disposes the custom header it replaces first, and a
host that has not built its own header yet runs no factory.

### Themes and tool expansion

`theme` returns the UI's live theme, which reads the host's later selections.
`get_all_themes` lists `ThemeInfo` records in the host's order, with an absent `path`
for a theme without a file. `get_theme` loads a theme by name without selecting it and
returns `None` for an unknown name. `set_theme` selects a theme with
`ThemeSelection::Name` or a host-supplied `ThemeSelection::Theme` and returns the host's
`ThemeSetResult`, whose `error` explains a selection that did not apply; a theme the
author built itself cannot be passed back and returns an error. A host rejection is an
error, not a failed selection. `get_tools_expanded` and `set_tools_expanded` read and
change the tool output expansion.

`Theme::get_thinking_border_color` and `Theme::get_bash_mode_border_color` return a
`BorderColor` callback that asks the theme's owner to color text with the border color
of a `ModelThinkingLevel` or of the shell mode. The level-to-color choice stays with the
host's theme, and each call reads the theme the handle currently observes.

### Terminal input

`on_terminal_input` adds a listener that sees each raw chunk of terminal input before
the focused component does. It runs synchronously with a callback emitter, like the
other synchronous callbacks, and returns `None` to leave the chunk as it is or a
`TerminalInputResult`: `consume: Some(true)` stops the chunk, `data` replaces it for the
listeners after it, and absent members leave it unchanged. When both are set,
consumption wins. A listener's error reaches the host with its message. `unsubscribe`
on the returned `Subscription` removes the listener; dropping the subscription keeps
it. The host owns the order of listeners and the delivery of what remains.

### Dialogs

`select`, `confirm`, `input` and `editor` ask the host for an answer and return a
future that owns its operands, so it may outlive the handle that started it:

| Method | Request | Dismissal |
| --- | --- | --- |
| `select` | A title and the ordered choices, repetitions and empty entries included. | `None` |
| `confirm` | A title and a message. | `false` |
| `input` | A title and an optional placeholder. | `None` |
| `editor` | A title and optional prefilled text. | `None` |

`ExtensionUIDialogOptions::timeout` reaches the host's dialog as the supplied
milliseconds, without a guest timer or a changed value. A supplied `signal` gets a
host signal of its own for that dialog: an abort before the call, or while the dialog
waits, reaches the host, which decides what the dialog answers. The guest adds no
deadline or dismissal of its own. When the dialog returns, its listener on the
caller's signal is removed, so the caller can keep the signal, abort it later or
pass it to another dialog. An absent placeholder or prefill stays distinct from an
empty one, and an empty answer stays distinct from a dismissal. A host rejection
resolves to its message; the guest never turns it into a dismissal.

## Resources and their native counterparts

Each guest handle wraps one resource from `crates/maestro-extensions-wasm/wit/host.wit`.
The host answers it with the native owner in the right-hand column, so the guest
holds no copy of the UI logic.

| Resource | Guest type | Native counterpart |
| --- | --- | --- |
| `extension-ui-context` | `ExtensionUIContext` | `maestro_extensions::ExtensionUIContext` |
| `theme` | `Theme` | `maestro_theme::LiveTheme` over `maestro_theme::Theme` |
| `tui` | `TUI` | `maestro_tui::TUI` |
| `footer-data` | `FooterData` | `maestro_extensions::footer_data_provider::ReadonlyFooterDataProvider` |
| `tool-invalidator` | `ToolRenderContext::invalidate` | `maestro_tools::ToolRenderContext::invalidate` |
| `subscription` | `Subscription` | `maestro_extensions::Subscription` |
| `callback-emitter` | `CallbackEmitter` | `maestro_extensions::CallbackEmitter` |
| `abort-signal` | `AbortSignal` | The caller's signal, followed by a host signal for each dialog. |

Components cross as `callback` resources: a guest `Component` becomes a
`maestro_tui::Component` on the host, and a factory's optional disposal becomes the
native `ExtensionComponent` disposal. A terminal input answer becomes a native
`InputListenerResult`.

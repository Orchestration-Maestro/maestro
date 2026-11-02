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
and returns an `ExtensionComponent`: the component, optional disposal and expansion
callbacks, and an optional implicit width. See [factory component capabilities](#factory-component-capabilities)
for callback ownership and errors. The host
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

### Host widgets

`Text`, `Box`, `Container`, `Spacer`, `Markdown` and `DynamicBorder` are widgets the host
builds and keeps; each facade forwards to the host's widget. Building one needs no UI, so
an extension may build widgets in any mode, and only showing them goes through the UI.
`From<&Text>` and the other conversions make a widget a `Component`, which a factory
returns or a container holds; the host keeps such a child as the widget itself.

| Widget | Operations beside `render` and `invalidate` |
| --- | --- |
| `Text::new(text, padding_x, padding_y, custom_bg_fn)` | `set_text`, `set_custom_bg_fn` |
| `Box::new(padding_x, padding_y, bg_fn)` | `add_child`, `remove_child`, `clear`, `set_bg_fn` |
| `Container::new()` | `add_child`, `remove_child`, `clear` |
| `Spacer::new(lines)` | `set_lines` |
| `Markdown::new(text, padding_x, padding_y, theme, default_text_style)` | `set_text` |
| `DynamicBorder::new(color)` | none |

A container or box renders host widgets and authored components in insertion order; the
same component may be added more than once, `remove_child` removes its first occurrence and
an absent child changes nothing. A removed child stays usable. An authored component may
render host widgets from its own `render`, which runs while the host renders its parent.

A `Style` styles text: `Style::new` wraps an authored function, which the host calls while
it renders. `get_markdown_theme` returns a `MarkdownTheme` whose styles read the live theme
and whose `Highlight` is the host's code highlighting; those go back to the host as its own
when a widget uses them. An authored `MarkdownTheme` may supply its own styles, highlighting
and code-block indent. The first failure of an authored style or highlighting is the next
`render` or `invalidate` error of the widget that uses it. `DynamicBorder` without a color
paints with the live border color.

`SelectList` shows `SelectItem` choices: `set_filter` keeps the choices whose value starts
with the filter, ignoring case; `set_selected_index` clamps to the rows; `get_selected_item`
reads the selection; and `set_on_select`, `set_on_cancel` and `set_on_selection_change`
replace or remove the handlers its input runs. `SelectListLayoutOptions` sets the narrowest
and widest primary column and may replace the primary truncation with an authored
`TruncatePrimary`. `SettingsList` shows `SettingItem` rows whose `values` cycle and whose
`Submenu` opens a component with a `SubmenuDone`; closing it with a value reports the change
to the list's `SettingChange` callback, and `update_value` changes a value without reporting
it. `BorderedLoader` builds the application's loader from a lent `TUI` and `Theme`, so it is
built inside a factory; its `signal` reports a cancellation, `set_on_abort` replaces the
handler a cancellation runs, `dispose` stops the animation and its rows take children. A
loader built with `cancellable: Some(false)` ignores input. `get_select_list_theme` and
`get_settings_list_theme` lend the live theme's styles; a `RowStyle` also receives whether
its row is selected. The first failure of an authored handler or style is the next `render`
or `handle_input` error of its widget.

`KeybindingsManager` is a manager the host keeps. `KeybindingsManager::new` and
`KeybindingsManager::create` build one of the application's keybindings, with user bindings
and the file `reload` reads, or loaded from `keybindings.json` in an agent directory;
`from_definitions` builds one of supplied `KeybindingDefinition`s; `get_keybindings` returns
the keybindings the host's terminal uses. A manager matches input, reads keys, definitions,
conflicts, user, resolved and effective bindings, and replaces its user bindings; a manager
without a file keeps its bindings on `reload`. `key_text`, `key_hint` and `raw_key_hint`
format keybinding hints with the host's keybindings and live theme, and `get_editor_theme`
lends the live editor styles.

`Editor::new` builds the host's multiline editor on a lent terminal, with an `EditorTheme` and
`EditorOptions`; `CustomEditor::new` builds the application's editor with a keybindings
manager. An editor reads and replaces its text, lines, cursor, history, padding, completion
rows, submission and focus, and takes submission and change handlers; replacing or inserting
text runs the change handler first. A custom editor also offers input to its
`set_on_extension_shortcut` interceptor, which consumes the input by answering `true`, then
runs its interrupt, exit and image-paste handlers and the `on_action` handlers of the
application actions its manager binds; replacing an action keeps its place.

The [authored editor and completion contracts](#authored-editor-and-completion-contracts) describe `EditorComponent`,
`AutocompleteProvider` and their optional operations. The host-backed editor facades implement
the editor traits, and [an editor factory](#editor-factories) replaces the host's
editor with an authored one.

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

### Caller-owned cancellation

`AbortController::new()` creates a signal that is not aborted. Pass
`controller.signal()` in `ExtensionUIDialogOptions::signal`, then call `controller.abort()` to cancel
operations using that signal. Clones share the cancellation state; a signal stays
usable after its controller is dropped. Aborting twice does nothing the second
time. Listeners run synchronously in registration order, and the first listener
error stops delivery and is returned to the caller. Listeners added after an
abort are not invoked, and a listener's callback emitter has no host channel.

The controller owns no timer. The
[timed dialogs example](../../examples/extensions/timed_confirm.rs) pairs it with
`ExtensionAPI::native_io().sleep(...)` and drops the unfinished wait when the
dialog answers first. The host still owns dialog dismissal and countdown display.

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

## Authored editor and completion contracts

The guest library defines `EditorComponent` and `AutocompleteProvider` for authored
editors and completion providers. An editor factory installs an authored editor in
place of the host's editor, and a provider factory stacks an authored provider on the
host's provider. The host-backed `Editor` and `CustomEditor` facades implement the editor
traits.

## Editors

Implement `ComponentView` for rendering and invalidation, then `EditorComponent`
for text access, input, submission/change callback setters and a change-callback
getter. Its required
`handle_input` is separate from `ComponentView`'s optional input handler. When an
editor is also used as an ordinary component, delegate that handler with
`EditorComponent::handle_input(self, data, emitter)`.

Every operation is synchronous and returns `ExtensionResult`. `CallbackEmitter`
is `Copy`: reuse it across several editor operations in one callback. Its copies
borrow the same frame and cannot outlive that invocation. Retain callbacks and
editor state, not emitters. Callbacks run after releasing any mutable borrow of editor state, so a
handler can call back into the same editor.

History, cursor insertion, expanded text, completion-provider installation,
border color and padding/completion-row setters are optional. Their defaults
return `Ok(None)`. `Some(())` means an operation ran; `None` means it is absent.
An expanded-text reader that gets `None` can fall back to `get_text`. An error is
neither absence nor a successful no-op.

## Completion providers

`AutocompleteProvider::get_suggestions` returns an asynchronous `ExtensionFuture`.
It receives the edited lines, a `CursorPosition` and `CompletionOptions`. Cursor
columns are UTF-8 byte offsets. The options borrow an `AbortSignal`; clone the
signal when keeping it independently. `force: None`, `Some(false)` and
`Some(true)` remain distinct. Providers choose how to react to cancellation.

`AutocompleteSuggestions` holds candidates and the prefix they replace.
`Ok(None)` means no suggestions; `Ok(Some(...))` with an empty candidate list is
a separate result. Candidate values, labels, descriptions and prefixes remain
literal, including empty strings and newlines.

`apply_completion` is synchronous and returns `CompletionResult`, with the edited
lines and cursor. `should_trigger_file_completion` is also synchronous. Its
default `Ok(None)` leaves the decision to the caller; `Some(false)` and
`Some(true)` are explicit decisions. Provider errors remain errors.

A wrapper can retain an `Rc<dyn AutocompleteProvider>` and delegate requests to
it.

`ExtensionUIContext::add_autocomplete_provider(factory)` stacks an
`AutocompleteProviderFactory` on the host's current provider. The factory receives that
provider and returns the provider the host uses from then on. The host keeps the factory
and runs it again whenever it rebuilds its provider stack, so each factory receives the
provider the factories before it returned. The host runs the factory before the request
returns, and the factory's error is the request's error.

A provider the host lends to a factory or to an editor forwards each request to the
host. A suggestion request keeps the caller's own signal when the host lent that signal,
so a provider that keeps the signal sees a later cancellation. Applying a completion and
the file-completion decision are synchronous, and the host provider may call back into
the extension before it answers. An authored provider's failure in an operation whose
native signature cannot return an error is reported by the provider's next suggestion
request.

The host's `Editor` and `CustomEditor` install a provider with
`set_autocomplete_provider`; an authored editor without the operation answers `None`.

## Editor factories

`ExtensionUIContext::set_editor_component(Some(factory))` replaces the host's editor
with the editor `factory` builds. The host runs the factory before the request
returns, and its error is the request's error. `None` restores the host's own editor.
`get_editor_component()` returns the factory that is current, or `None` while the
host's own editor is in use. A factory this extension set comes back as the same `Rc`
it supplied. A factory another extension set comes back as a factory the host runs:
calling it, usually from this extension's own factory, builds that extension's
editor. The result is an `EditorFactoryOutcome` like any other. Its editor reaches
the other extension's editor through the host and keeps the operations that editor
lacks absent. A wrapping factory can return the outcome as it is or delegate to it.

An `EditorFactory` receives the terminal, the editor styles, the keybindings manager
and a borrowed emitter. The manager keeps the supplied application reload even after
the factory returns or the editor is replaced; clones observe the same bindings.
Its effective configuration is an owned snapshot of resolved actions.
It returns an `EditorFactoryOutcome`:

- `Editor` holds an editor with only the editor operations.
- `CustomEditor` holds a `CustomEditorComponent`, an editor built on the application's
  `CustomEditor`. Its `custom_editor` names that custom editor. The host shares the
  custom editor's application actions and its interrupt, exit, image-paste and
  shortcut callbacks. Input the authored editor passes on to the custom editor runs
  those actions, and the custom editor's focus is the authored editor's focus.

`Editor` and `CustomEditor` implement the matching traits. A factory can return one as
it is, or wrap one and delegate to it, as a modal editor does for the keys it does not
keep. [`examples/extensions/modal_editor.rs`](../../examples/extensions/modal_editor.rs)
wraps the custom editor this way.

The host calls the authored editor through its operations. It renders and delivers
input through the editor's component, and the input reaches
`EditorComponent::handle_input`. Submission and change callbacks the host installs
are host callbacks lent to the editor. A change callback the editor returns can be
called by the host. The host keeps the submission callback it installed; reading it
does not enter the editor. Optional operations keep their absence, so the host skips
a border, padding or expansion the editor lacks. When an operation whose native
signature cannot return an error fails, the editor's next rendering reports that
failure.

## Factory component capabilities

`ExtensionComponent` returns the component, optional `dispose` and `set_expanded`
callbacks, and an optional `width`. Set unused fields to `None`.
`ComponentExpansionHandler` receives a boolean and a borrowed `CallbackEmitter`.
It runs synchronously, can run repeatedly after the factory returns, and returns
its original error to the host. A host can also call it before its UI setter
returns. Cloned native handles share the callback; removing the shown component
does not invalidate an independently retained handle.

The width crosses the boundary unchanged, including absent, zero and fractional
values. It is an implicit overlay width for a host that supports custom overlays;
this record does not install an overlay or choose its layout.

| Protocol value | Native counterpart | Ownership |
| --- | --- | --- |
| `factory-component` | `ExtensionComponent` | Transfers the component, optional disposal and expansion callback through the existing callback owner; carries width by value. |
| `set-expanded` callback | `UIHostCall<bool, ()>` | Retained independently; each call borrows its own emission frame. Its final native handle releases the authored capture. |
| `set-editor-component`, `get-editor-component` | `ExtensionUIContextPort::set_editor_component`, `get_editor_component` | The host retains the factory identity while the factory is installed, and answers it back by identity. |
| `invoke-editor-factory`, `editor-built` | `EditorFactory`, `EditorFactoryOutcome` | Lends the terminal, styles and keybindings for the call; transfers the editor's component and editor identities, and a shared handle of the custom editor it extends. |
| `invoke-editor` | `EditorComponent` | One request per editor operation, each with its own emission frame. |
| `text-callback` | `TextCallback` | A host callback lent to the editor; dropping it keeps the host's callback. |
| `widget.border-color`, `widget.share` | `Editor::border_color`, the widget itself | Lends the host editor's border style; shares the widget handle with the host. |
| `current-editor-factory`, `editor-factory.build` | `EditorFactory` another extension set | Lends that factory; each build lends the terminal, styles and keybindings and transfers the built editor's widget, and whether it extends the custom editor. |
| `widget.foreign-editor` | `EditorComponent` of that built editor | One request per editor operation; `absent` answers an optional operation the editor lacks. |
| `add-autocomplete-provider`, `invoke-provider-factory` | `ExtensionUIContextPort::add_autocomplete_provider`, `AutocompleteProviderFactory` | The host retains the factory for later rebuilds; each call lends the current provider and transfers the returned provider's identity. |
| `invoke-provider` | `AutocompleteProvider` | One request per provider operation; a suggestion request owns its lent signal. |
| `completion-provider` | `AutocompleteProvider` | A host provider lent to a factory or editor; dropping it keeps the host's provider. |
| `widget.set-autocomplete-provider` | `Editor::set_autocomplete_provider` | Transfers an authored provider to the host editor. |

Disposal, component removal and final callback release remain separate effects.
Neither calling expansion nor dropping its handle invokes disposal.

### Retained frame writer

A factory's `TUI::full_redraws` reads the original host frame writer's count on
every call. Clones retain the same host capability after the factory returns;
releasing the last clone releases that capability, not the host terminal.

The other `TUI` methods forward to that same frame writer. `add_child`,
`remove_child`, `clear`, `invalidate` and `render` work on the writer's own
components, which may be host widgets or authored components. `set_focus` and
`show_overlay` accept either kind; an authored component keeps one host identity
per `TUI` value, so focusing the component an overlay shows moves focus to that
overlay. `show_overlay` returns an `OverlayHandle`. The hardware-cursor and
clear-on-shrink flags, `hide_overlay`, `has_overlay`, `start`, `stop` and
`request_render` call the host operation of the same name.

`add_input_listener` takes a `TerminalInputHandler` and returns a `Subscription`;
`remove_input_listener` removes the listener added as that same handler.
`set_on_debug` replaces or removes the callback for the debug key. These callbacks
cannot report a failure while the host runs them: a failed listener consumes its
input chunk, and the next request on the frame writer returns the first failure.

`TUI::terminal` returns the `Terminal` the writer draws on. Its methods read the
live size and the Kitty keyboard protocol flag, write output, move and show or hide
the cursor, clear, set the title and progress indicator, and start or stop the
terminal. `drain_input` resolves once the host's drain finishes; `None` keeps the
terminal's default limits. `start` takes an input `TextHandler` and a resize
`WidgetNotice`; their failures surface like those of input listeners.

| Protocol value | Native counterpart | Ownership |
| --- | --- | --- |
| `tui.writer` | `TUI` as a container widget | Lends the original frame writer once per `TUI` value; it keeps the authored components it lent. |
| `widget.writer`, `writer-request`, `terminal-request` | `TUI` and `TUI::terminal` methods | One request per operation; focus, overlay and visibility requests may call back into the guest. The host holds focus and overlay targets only while the frame writer does, so refocusing a component first asks `lent-live` and lends it again once released. |
| `widget.add-input-listener` | `TUI::add_input_listener` | The host keeps the listener until its subscription is unsubscribed. |
| `widget.drain-input` | `Terminal::drain_input` | The host owns the drain future until it settles. |

### Authored focus

A `ComponentView` can return `Some(self)` from `focusable` when it implements
`Focusable`. Its getter and setter run synchronously with a borrowed
`CallbackEmitter`. `is_focusable` tests whether the capability exists, not whether
its flag is currently true. The host calls the setter on focus and blur, including
repeated assignments. Ordinary callback failures surface on the next fallible
render, as they do for input and invalidation.

Emit `CURSOR_MARKER` at the text cursor only while focused. The host removes the
zero-width marker and positions its hardware cursor there. This does not install
an overlay or make a nonfocusable view focusable.

`Editor` and `CustomEditor` keep their host focus capability through either
`Component::from(&editor)` or `Component::new(editor.clone())`. Both views read and
change the original editor's flag, rather than keeping a second guest flag.

| Protocol operation | Native counterpart | Ownership |
| --- | --- | --- |
| `component-focused`, `component-set-focused` | `Component::focusable`, `Focusable::set_focused` | Borrows the retained component and the current emission frame; focus state stays with the author. |
## Live themes

`theme()` retains the live theme used by the host's widgets, without acquiring a UI.
Call it after the extension host starts. Its methods report
`Theme not initialized. Call initTheme() first.` until the host publishes a palette.
Retained aliases read later name, path, provenance, palette and color-mode changes.
Dropping an alias releases that resource, not the host's palette.

`ThemeColor` and `ThemeBg` name borrowed string keys. `fg` and `get_fg_ansi` accept
foreground keys; `bg` and `get_bg_ansi` accept background keys. Strings cross unchanged.
Unknown keys return the host's lookup error rather than choosing a fallback color.
`ExtensionUIContext::theme()` still reads that UI owner's live theme; a renderer keeps
the independently supplied theme it received.

| Protocol value | Native counterpart | Ownership |
| --- | --- | --- |
| `global-theme` | `ExtensionWidgets::theme`, `LiveTheme` | Lends the widget host's live slot through the existing retained theme resource. |

## Shared overlay placement

`OverlayOptions` holds the placement fields, with `SizeValue` for cells or
percentage text, all nine `OverlayAnchor` positions and uniform or per-edge
margins. The host toolkit resolves those values; the guest does not lay out an
overlay.

`OverlayOptionsHandle::new` creates one host-owned placement record. Clones share
it: `get` reads the current native fields and `set` replaces them on that same
record. A replacement preserves the record's executable visibility policy.
Dropping a guest alias releases that capability, not another native owner's
reference to the record.

`set_visible` replaces or removes the record's visibility check. The host runs it at
each render with the terminal's columns and rows; placement replacements keep it.
A failed or trapped check counts as hidden for that render. Once a custom component
uses the record, the host reports each such failure as an extension error for the
`overlay.visible` event, attributed to the extension that set the check, and goes on
rendering.

| Protocol value | Native counterpart | Ownership |
| --- | --- | --- |
| `overlay-options` | `Rc<RefCell<maestro_tui::OverlayOptions>>` | Retains one shared record; `get`/`set` carry only placement, not executable code. |
| `overlay-options.set-visible` | `OverlayOptions::visible` | The host keeps the authored check until it is replaced or the record is dropped. |
| `overlay-handle` | `Rc<RefCell<Box<dyn OverlayHandle>>>` | Shares the shown overlay's handle; dropping it does not hide the overlay. |

## Custom components

`ExtensionUIContext::custom` shows the component a `CustomFactory` builds and
resolves to the first value passed to its `CustomDone`, or `None` when the host
closes the component without one. The value stays in the guest, so it need not
be serializable. A factory returns `CustomFactoryOutcome::Ready` with the
component, or `Async` with a future that owns its captures; only custom factories
may build asynchronously.

`CustomUIOptions` keeps absent fields absent. With `overlay` set, the host shows the
component as an overlay. `overlay_options` is a shared `OverlayOptionsHandle` or an
`OverlayOptionsResolver` that the host calls after the component is built. With
neither, the host uses the component's own `width`. After the overlay is shown, the
host calls `on_handle` with its `OverlayHandle`. The handle's `hide`, `set_hidden`,
`is_hidden`, `focus`, `unfocus` and `is_focused` forward to the host's handle.

| Protocol value | Native counterpart | Ownership |
| --- | --- | --- |
| `extension-ui-context.custom`, `custom-ui-options` | `ExtensionUIContextPort::custom_component`, `CustomUIOptions` | The factory, resolver and handle callback live until the custom operation ends. |
| `custom-close` | `CustomClose` | Closes the component; only the first accepted close commits the guest's value. |
| `invoke-custom-factory`, `invoke-custom-future` | `CustomComponentFactory`, `CustomFactoryOutcome` | The factory prefix borrows its emission frame; an asynchronous future owns its captures. |

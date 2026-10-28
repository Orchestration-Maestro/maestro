# Build a terminal view

Start with the [extension project guide](../extensions.md#create-the-project),
then choose the smallest UI operation that fits. Use `select`, `confirm`, `input`
or `editor` for a host dialog. Use `custom` when you need to own the rows and
input handling. Check `ctx.has_ui()` before opening an interactive view; a
component-enabled host and an interactive UI are separate requirements.

This guide connects the native [component contract](../terminal/components.md)
to the [guest UI](guest_ui.md). Extension authors use
`maestro-extensions-wasm`; native toolkit implementations use `maestro-tui`.
Their Rust interfaces differ. Do not import the native toolkit into a guest
extension.

## Choose the building blocks

| Need | Reuse |
| --- | --- |
| Padded text, vertical layout or blank rows | `Text`, `Box`, `Container`, `Spacer` |
| Styled prose and highlighted code | `Markdown` with `get_markdown_theme()` |
| A framed heading | `DynamicBorder` |
| One-line or multiline editing | `Input` or `Editor` in the native toolkit; the guest exposes `Editor` |
| A command choice or value toggles | `SelectList` or `SettingsList` |
| Work that can be cancelled | Guest `BorderedLoader`; native `CancellableLoader` |

The [widget reference](../terminal/widgets.md) explains native construction,
input and cache behavior. Guest widgets are retained host resources, not another
layout engine: convert a widget with `Component::from(&widget)` when returning it
or adding it to a container. See [guest host widgets](guest_ui.md#host-widgets)
for constructors and conversions.

For native inline images, use [`Image`](../terminal/images.md) with the caller's
`TerminalImage` state. Capability detection selects Kitty, iTerm2 or a text
fallback. Kitty payloads must already be PNG. Invalidate after changing terminal
capabilities or cell measurements; the image caches same-width output. Its
fallback text is not clipped, so check narrow layouts yourself. The image
reference describes protocol constraints; a captured byte stream does not prove
that a terminal decoded the pixels.

## Keep every row within its width

A native `Component::render` receives a `usize` width and returns rows. A guest
`ComponentView::render` receives a `u32` width and a callback emitter and returns
`ExtensionResult<Vec<String>>`. Both must respect the supplied number of cells.
A box's padding reduces the width its children receive, potentially to zero.

1. Measure display cells with `visible_width`, not UTF-8 bytes or character count.
2. Clip dynamic rows with `truncate_to_width`, or wrap styled prose with
   `wrap_text_with_ansi`.
3. Reserve cells for borders and labels before clipping content. Handle widths
   smaller than that decoration without unsigned subtraction underflow.
4. Reapply styling on each row. The frame writer appends style and hyperlink
   resets, so a color on one row does not style the next.

The [text reference](../terminal/text.md) defines grapheme and escape handling.
Guest text helpers take the extension API and return fallible host answers; see
[`custom_footer.rs`](../../examples/extensions/custom_footer.rs) for measuring
and clipping a styled row. The frame writer rejects oversized ordinary rows
rather than fixing them. Test at zero width and at widths smaller than a border,
with combining characters, wide characters and styled text.

## Give input and focus to the right child

A native component receives input only if `input_handler()` returns a handler;
it receives focus updates only if `focusable()` returns a focusable target.
These capabilities are independent. A guest view implements `handle_input` and
can return `Some(self)` from `focusable()` when it implements `Focusable`.
Request key-release events only when your view needs them; native components
otherwise default to no releases.

Forward input to the retained list or editor instead of reimplementing its
bindings. For authored controls, use the host's key helpers and injected
keybindings, not literal escape-byte comparisons. The [key decoder](../terminal/keys.md)
and [application keybindings](../keybindings.md) cover the two layers.

A wrapper containing an editor must forward focus as well as input. Otherwise
the editor can accept text without placing the terminal cursor for an input
method's candidate window. Native `FocusFlag::forwarding` retains a child and
forwards every assignment; `forwarding_all` supports several targets. Guest
wrappers forward `Focusable` getters and setters to the intended editor. Emit
`CURSOR_MARKER` at the text cursor only while focused. It occupies no cells;
the writer removes it and uses its location for the hardware cursor, subject to
the writer's [cursor policy](../terminal/rendering.md#defaults).

The existing [focus author fixture](../../crates/maestro-extensions-wasm/src/tests/focus_author.rs)
shows guest focus and editor conversions. Its tests check setters and markers,
not physical input-method placement on a real terminal.

## Build, show and release

`ctx.ui()?.custom` gives its factory the frame writer, live theme, application
keybindings, typed `done` callback and a borrowed emitter. Return
`CustomFactoryOutcome::Ready(ExtensionComponent { ... })` for a synchronous
build, or `Async` with an owned future for asynchronous construction. Set unused
`dispose`, `set_expanded` and `width` fields to `None`.

The first accepted `done(value)` closes the view and commits its result. The
awaited operation returns `Some(value)`; closing without a value returns `None`.
For an optional result, `Some(None)` still differs from no committed result.
Dropping the awaiting future does not close the view. Put cleanup in `dispose`:
stop animation, release pending work and unsubscribe listeners there. Dropping a
subscription alone does not unsubscribe it.

Use a fresh view for each opening so its state and cleanup belong to that
opening. A component handle can outlive removal, but that does not mean the host
still displays it. [Factory ownership](guest_ui.md#factory-component-capabilities)
and [custom completion](ui.md#custom-components) describe those separate effects.

### Show a floating view

Use `CustomUIOptions` with `overlay` enabled when the view should float over
retained content. Supply an `OverlayOptionsHandle`, or a resolver that runs after
the build, for placement. Cells and percentage strings, nine anchors, margins,
offsets and explicit row/column positions are resolved by the toolkit. See
[overlay placement](../terminal/overlays.md#placement) rather than calculating a
second layout in the extension.

Clones of an options handle share edits. A visibility callback can hide the
view on small terminals; a failing callback counts as hidden and is reported as
an extension error. `on_handle` receives the shown overlay's handle. Use
`set_hidden` for temporary hiding, `focus`/`unfocus` for explicit focus changes
and `hide` for removal. Dropping the handle does not remove the overlay. Closing
a custom operation and hiding an overlay are distinct operations; finish the
custom operation through its `done` callback too.

## Keep styling and bindings live

Use the theme supplied to the factory or renderer. Style foregrounds and backgrounds with named theme keys, not literal colors. The [theme guide](../theme.md)
lists the palette contract. `get_markdown_theme`, `get_select_list_theme` and
`get_settings_list_theme` reuse host styles. A retained live theme observes later
publications; a prepared theme instance stays fixed.

Keep the injected keybindings with the view. Their clones share the application
manager, including its reload. Use their hint operations for configurable keys;
do not describe a hard-coded shortcut as the user's current binding. A custom
editor needs the application manager, not an independently constructed toolkit
manager. [Editor factories](guest_ui.md#editor-factories) describe the guest
argument types and the corresponding native capabilities.

When state changes, invalidate cached rows and request a render with
`tui.request_render(false)`. Invalidation alone does not schedule a frame.
Include width in any cache key. Clear theme-derived cached output on
invalidation, and rebuild text that already contains old color escapes; clearing
only a child's layout cache cannot recolor stored strings. Forward invalidation
to retained children. A view that computes styling on every render needs no
second themed cache.

## Adapt a tested recipe

These recipes use shipped sources and existing contract fixtures. Build and
load an example only through the [extension setup](../extensions.md); a `.rs`
file is not a loadable extension. The [example index](../../examples/extensions/README.md)
provides component build commands and host prerequisites.

### Select one item

Follow [`preset/selector.rs`](../../examples/extensions/preset/selector.rs):

1. Create `SelectItem` records with stable values and display labels.
2. Build a `SelectList` inside a `Container` with host borders and a heading.
3. Connect selection to `done(Some(item.value.clone()))` and cancellation to
   `done(None)`.
4. Delegate render and invalidation to the container, and input to the list.
   Request a render after input.

Keep selection separate from applying it. The preset command does nothing on
dismissal; applying a real preset can change model, thinking level and active
tools. Use the host's `select` dialog when a custom frame adds no useful behavior.

### Wait with cancellation

Follow [`qna.rs`](../../examples/extensions/qna.rs): construct a `BorderedLoader`
with the supplied writer and theme, and connect its abort callback to completion
without a result. Pass its signal to the actual operation, not only to the
spinner. Keep the running future in owned state and release it in disposal along
with the loader. The example uses a host-owned event tail to poll the extraction;
it does not retain a borrowed callback emitter across an await.

Only replace editor text after successful completion. The question extractor
leaves input unchanged on cancellation; a real extraction sends assistant text
to the selected provider and can incur charges. The controlled run below uses a
loopback provider with disposable session files and no live credentials.

### Change several values

Follow [`tools.rs`](../../examples/extensions/tools.rs): build `SettingItem`
records from the current inventory, use `SettingsList` with host styles and
connect its change callback to the operation being configured. Connect cancel to
`done(())` and forward input, rendering and invalidation to the retained list.

The example applies each tool change immediately and appends a session entry;
Escape closes without rolling back changes. Its session-start and tree handlers
restore the newest configuration on the current branch, filtering removed tools.
Do not promise save/cancel semantics unless your own settings operation supplies
them.

### Report status or working activity

Use [`status_line.rs`](../../examples/extensions/status_line.rs) for keyed footer
status: `set_status` replaces one key, and `None` clears it. Choose a key owned
by your extension. The example reads the current UI theme on each event before
formatting its ready or turn-count text.

[`working_message_test.rs`](../../examples/extensions/working_message_test.rs)
requests working text and a one-frame indicator on session start. For your own
frames, resolve colors through the supplied theme; the example's literal colors
are illustrative, not a theme to reuse. `WorkingIndicatorOptions.frames` is
verbatim text: an empty list hides the indicator, one frame is static, and `None`
to `set_working_indicator` asks for the host default. Animation belongs to the
host, not a guest timer. Normal working, retry and compaction loaders have
separate policies; see [interactive mode](../chat/interactive-mode.md).

### Keep content beside the editor

Follow [`widget_placement.rs`](../../examples/extensions/widget_placement.rs):
check UI availability, then set named line widgets. Absent placement means above
the editor; `BelowEditor` requests below. Clear a key with `set_widget` and
`None`. For changing content, use `set_widget_factory` with a width-safe
component. Replacing or removing that keyed factory widget calls its disposal;
it does not dispose unrelated widgets.

### Replace the footer

Follow [`custom_footer.rs`](../../examples/extensions/custom_footer.rs): use
`set_footer` with a factory that receives the writer, theme and live `FooterData`.
Read branch, statuses and provider count through that supplied data; compute
usage from assistant messages on the active session branch. Clip the composed
row to the supplied width. Subscribe to branch changes to request renders, and
unsubscribe in disposal. `set_footer(None)` restores the default footer and
disposes the replacement. A custom footer owns its displayed content; the host
does not automatically append the default footer's fields to it.

### Wrap the application editor

Use the tested `Modal` wrapper and `custom` factory in the
[editor factory fixture](../../crates/maestro-extensions-wasm/src/tests/editor_factory_author.rs)
as the minimal delegation recipe:

1. Construct `CustomEditor::new` from the factory's writer, editor theme,
   keybindings and `EditorOptions`.
2. Wrap it in an `EditorComponent`. Handle only input your view owns and delegate
   other input to `EditorComponent::handle_input` on the retained custom editor.
3. Forward text, submit/change callbacks, render and invalidation. Forward any
   optional editor operations your wrapper supports without turning absence or
   failure into a fabricated success.
4. Implement `CustomEditorComponent::custom_editor` and return
   `EditorFactoryOutcome::CustomEditor`. Install with
   `set_editor_component(Some(factory))`; pass `None` to restore the host editor.

That outcome lets the host attach its application actions, interrupt, exit,
image-paste and shortcut callbacks. A plain `Editor` outcome does not expose
that custom-editor capability. For a wrapper with a separate component view,
forward focus to the retained editor as well. The fixture demonstrates consuming
one key and delegating others; the packaged insert/normal-mode example is still
tracked in [the modal editor example issue](https://github.com/Orchestration-Maestro/maestro/issues/2242).

## Check a recipe without a live account

Run these commands from a Maestro checkout with its pinned toolchain and tools
installed. They execute existing tests, not an installed extension. The guest
suite covers both in-process and built-component adapters, including the
recipes and editor/focus/overlay contracts. The native command covers question
extraction with controlled provider and terminal inputs. No live keys or paid
calls are required.

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- qna_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo test --locked \
  -p maestro-tui --doc
```

The native doctests execute the component and widget samples linked above. For
real-terminal qualification, use the [terminal demonstrations](../terminal/examples.md)
in a disposable terminal. Controlled render tests cannot establish physical IME
placement or decoded image appearance.

## Diagnose stale or oversized output

First check width accounting, then invalidation, then whether a render was
requested. The [frame writer logs](../terminal/rendering.md#errors-and-diagnostics) distinguish
full redraws from differential updates and report oversized rows. The native
adapter's [`MAESTRO_TUI_WRITE_LOG`](../terminal/native-terminal.md#write-log)
captures admitted explicit writes, not cursor, title, protocol or progress
operations. Use a disposable log destination: terminal text may contain private
session content. Log capture is not proof of what a terminal displayed.

# Browser input, timers and dialogs

[`Input`](../../crates/maestro-web/src/components/input.rs) is a native
single-line field. `Input::new(view, props)` mounts a wrapper holding an optional
label, the `<input>` element and an optional error line; place `root()` where the
field belongs and read `input()` for focus, selection and live text.

The seven input types are text, email, password, number, URL, telephone and
search; small, medium (default) and large sizes select the field height. The
browser owns editing, validation, selection and autocomplete. A non-empty label
names the field through `aria-labelledby`; a required labelled field appends the
localized required marker. A non-empty error shows below the field and marks it
`aria-invalid`. Empty identifiers, names, placeholders and numeric bounds leave
their attributes out.

`update(view, props)` changes the field in place. It keeps native edits while the
supplied value stays the same and assigns a changed supplied value. The input,
change, keydown and keyup callbacks receive the actual browser events, and each
delivery calls the most recently supplied callback. Dropping the `Input` releases
its listeners. Capture the owning component weakly in callbacks that reach it.

[`Timer`](../../crates/maestro-web/src/browser/timer.rs) owns cancellation of one
native timeout, interval or next-frame callback. Explicit `cancel` and drop revoke
future delivery; cancelling does not abort asynchronous work its callback started.

## Dialogs

[`dialog`](../../crates/maestro-web/src/dialogs/dialog.rs) mounts an open dialog
as native page content: a dimmed full-viewport backdrop holding a panel with the
caller's children and a Close button named by the localized `Close` text. A
closed `DialogProps` mounts nothing. The backdrop, the Close button and document
Escape each notify `on_close` once per request; clicks inside the panel stay
inside it. The panel carries `role="dialog"`, `aria-modal` and the literal CSS
width and height, and its first heading names it. The returned `DomRoot` owns the
Escape listener, so dropping it releases that route.

`dialog_header`, `dialog_content` and `dialog_footer` build the heading with its
optional description, the padded content column and the action row. Caller text
stays literal; it is never parsed as markup.

[`DialogBase`](../../crates/maestro-web/src/dialogs/dialog_base.rs) is the opened
dialog the built-in dialogs share. `open(content, component)` remembers the
focused element and mounts the dialog on the page body; the dialog keeps the
`component` state alive while it is open, so that component's callbacks can hold
it weakly. Closing by any route removes the dialog, restores the remembered focus
when that element is still connected, notifies `set_on_close`'s callback, then
releases the component. Opening an open dialog and closing a closed one change
nothing. Each open dialog closes once on one Escape. The standard panel is
`MODAL_WIDTH` by `MODAL_HEIGHT`.

[`PromptDialog::ask`](../../crates/maestro-web/src/dialogs/prompt_dialog.rs) shows
a one-line prompt at once and returns a future of the confirmed text. Enter and
Confirm return the live text; Cancel, Escape, the backdrop and Close return
`None`, so the future never stays pending after the prompt closes.
[`get_auth_token`](../../crates/maestro-web/src/utils/auth_token.rs) returns the
token stored under `auth-token` in local storage, or asks for it through a masked
prompt and stores the trimmed entry. A blank entry asks again; cancelling
returns `None` and stores nothing. `clear_auth_token` removes the stored token.
This is the browser-local token helper, not the daemon's one-time-key login.

## Models and providers

[`ModelSelector::open`](../../crates/maestro-web/src/dialogs/model_selector_dialog.rs)
lists every catalog model, then the models of the configured custom providers:
discovery servers contribute the models they report under the provider's name,
manual providers their stored models. A server that cannot be reached adds
nothing. Typed search text is lowercased and stripped of whitespace, matched
character by character in order against the provider, identifier and name, and
ranked by how few characters lie between the matches; equal scores keep list
order. Without search text the current model comes first and the rest follow by
provider. The Thinking and Vision filters keep reasoning and image-input models
and can be combined; `allowed_providers` limits the listed providers. Each row
shows the identifier, the current-model mark, the provider, capability icons, the
context and output limits, and the price.

The search field has focus when the picker opens. The arrow keys move the
highlight within the shown rows and Enter chooses it; key presses during text
composition are ignored. Real pointer movement returns to pointer navigation and
highlights the row under the pointer. Typing or switching a filter returns to the
first row at the top of the list. Choosing passes the model to `on_select`, then
closes the picker.

[`ProviderKeyInput`](../../crates/maestro-web/src/components/provider_key_input.rs)
mounts one provider's masked key entry. Save tests the typed key first: a known
provider completes a 200-token request against its test model, sent through the
CORS proxy when the stored proxy settings enable one and the provider and key
need it. Only a normal stop stores the key; other outcomes show the invalid badge
for five seconds. A provider without a test model stores the key without a
request. A stored key shows the saved mark and a masked placeholder, and Save
stays disabled until the key is edited again. Edits made during a save stay
pending. Changing the provider mounts a fresh entry, without carrying the previous
provider's unsaved key.

[`ApiKeyPromptDialog::prompt`](../../crates/maestro-web/src/dialogs/api_key_prompt_dialog.rs)
shows that entry for one provider and checks stored keys every 500 ms. It resolves
`true` once a key is stored and `false` when the dialog closes first; closing
stops the checks.

[`CustomProviderDialog::open`](../../crates/maestro-web/src/dialogs/custom_provider_dialog.rs)
adds or edits a custom provider. A new provider starts with its type's default
local address, and choosing another type starts again from that type's default.
Discovery types offer Test Connection, which lists the server's first five models
and how many more it found, or shows the failure. Save needs a name and an
address. It keeps an edited provider's identifier and stored manual models, gives
a new provider a random identifier, and stores an empty key as absent. A failed
save shows an alert and keeps the dialog and its edits.

[`ProvidersModelsTab`](../../crates/maestro-web/src/dialogs/providers_models_tab.rs)
is a [`SettingsTab`](../../crates/maestro-web/src/dialogs/settings_dialog.rs)
listing a key entry for every built-in provider and a card for every custom
provider. Discovery cards show checking, then the reported model count or
disconnection; Refresh checks again and alerts a failure. Edit and the Add
Provider menu open the provider dialog and reload the list after a save; Delete
asks for confirmation first.

## Settings, saved sessions and storage

A `SettingsTab` is page content with a localized tab name.
[`SettingsDialog::open`](../../crates/maestro-web/src/dialogs/settings_dialog.rs)
shows the caller's tabs in order, as a sidebar on wide screens and as tabs above
the content on narrow ones, and opens nothing for an empty list. Every tab stays
mounted while another one is shown, so switching keeps its edits. Closing removes
the dialog, then notifies `on_close`. `ApiKeysTab` lists a key entry for every
built-in provider. `ProxyTab` shows the stored proxy switch and address, defaulting
to off and `http://localhost:3001`; switching saves both values, the address is
editable only while the proxy is on, and a changed address is saved when the
field commits it.

[`SessionListDialog::open`](../../crates/maestro-web/src/dialogs/session_list_dialog.rs)
shows the stored sessions in storage order with title, date, message count and
usage. Dates read Today, Yesterday, a number of days ago within a week, or the
browser's local date; a time later than now reads Today. Each row's details are a
button, so Tab reaches a session before its delete control and Enter chooses it.
Choosing a session passes its identifier to `on_select` and closes. The delete
control never chooses its row: after confirmation it deletes the history and
metadata together and reloads the list. Closing without a choice passes each deleted identifier to `on_delete`.
A deletion still in progress reports after closing if its write succeeds.

[`PersistentStorageDialog::request`](../../crates/maestro-web/src/dialogs/persistent_storage_dialog.rs)
returns `true` at once when the browser already persists the page's storage.
Otherwise it explains the request and asks. Continue Anyway and closing return
`false`. After Grant Permission the application storage requests persistence and
the result is the browser's answer; an unavailable platform request, a refusal
and a failure each return `false` with their own console message.

## Message editor

[`MessageEditor::new`](../../crates/maestro-web/src/components/message_editor/browser.rs)
builds the message composer from `MessageEditorProps` and focuses its text field on
the first frame after the caller shows it. The editor owns its text and attachments:
`value`, `set_value`, `attachments` and `set_attachments` read and replace them, and
`update` changes the other operands without discarding an edit. The callbacks in
effect are always the last ones supplied.

Enter sends the untrimmed text with the attachments unless Shift is held; a draft
needs some non-whitespace text or an attachment, and nothing is sent while files
load or a response streams. Keys pressed during text composition do nothing.
While streaming, Abort replaces Send and Escape aborts. The caller clears the
draft after an accepted send.

The attachment control opens the file chooser, whose accepted types are only a
hint. Dropped files and pasted images are added too; other pasted content keeps
its native behaviour. A batch that would exceed `max_files` (10 by default) is
refused whole with an alert. Each file larger than `max_file_size` (20 MiB by
default) or failing to load gets its own alert, and the rest load in order. A
spinner replaces the attachment control while files load, and the chooser resets
after each batch. Every tile can be removed; additions and removals report the
complete list through `on_files_change`. The drop hint stays until the pointer
leaves the editor's box. A reasoning model adds the level menu, Off to High. The
model control focuses the text field first, so focus returns there, then calls
`on_model_select` on the next frame. `EditorControls` hides the attachment, model
or level control.

## Session view

[`AgentInterface::new`](../../crates/maestro-web/src/components/agent_interface/browser.rs)
shows a `BrowserSession`, a local agent over the browser message records: the
completed history, the streaming response and the message editor, with the
summed assistant usage below. Without a session it reads "No session set".
`update` with a different session moves the subscription to it, and dropping the
view ends the subscription and its observations. The view gives a session only
what it lacks: the page's task spawner, a stream that reads the stored proxy
settings on every request, and a key lookup that reads the nonempty stored
provider key, else the nonempty key of the stored custom provider of that name.

`send_message`, which the editor's Send also calls, ignores a blank draft without
attachments and any send while a response streams. A provider with no stored key
asks `on_api_key_required`; a refusal, or no callback, keeps the draft. The
`on_before_send` hook runs next and its failure also keeps the draft. Only then
is the draft cleared and the prompt started, carrying the original attachments
as a user message with attachments. A finished response moves into the history
once and leaves the streaming area.

The transcript follows new content until the reader scrolls up from a nonzero
offset more than 50 px from the bottom; coming within 10 px resumes it, and a
shrinking viewport is ignored as relayout. `set_auto_scroll` sets it directly and
`set_input` replaces the draft. With `on_cost_click` the usage totals are a
button. `on_model_select` replaces the built-in model selector, and the level
menu changes the session's reasoning level unless `controls.thinking` is off.
`show_theme_toggle` adds the appearance button beside the totals. Embedders can
import `ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW` from the SDK for the existing
read-write artifact runtime guidance.

[`ThemeToggle::new`](../../crates/maestro-web/src/components/theme_toggle/browser.rs)
is a keyboard-operable icon button named "Toggle theme", cycling light and dark,
then system when `include_system` is
set; without system in the cycle, system counts as light. The choice is stored
under `theme` in the page's local storage, and system removes it; an absent or
empty value shows system. The page root receives the pack's dark or light colors.
In system mode the button follows the platform's dark preference while it exists.
An unknown stored value shows light, with the sun icon, and the next activation
starts the cycle at light.

## Chat screen

[`ChatPanel::new`](../../crates/maestro-web/src/chat_panel.rs) registers the built-in
elements and styles, then builds the chat screen, which reads "No agent set" until
`set_agent` gives it a `BrowserSession`.
`set_agent` builds a new session view with every editor control and no appearance
button, and a new native artifacts panel that records into that session. It passes
the `ChatPanelConfig` key, pre-send, usage and model callbacks to the view and the
sandbox address to the panel. The session's tools become the panel's artifacts
tool followed by whatever `tools_factory` returns. The factory receives the
original session, view and panel, and a runtime provider factory. That factory
offers the session's current attachments, when there are any, and read-write
artifact access. `JavaScriptReplTool::local_tool` turns the JavaScript tool
into an entry of the browser-local session. The session view retains its own
artifacts tool renderer, so filename actions stay with that panel. Artifacts left
by the history are restored without opening the panel.

After that, a new artifact opens the panel; an edit does not. The panel's Close
control closes it, and while it is closed a count badge above the chat reopens
it. At 800 px of window width and above, an open panel takes half of the row.
Below that the panel fills the viewport above the backdrop, without an inset,
border or rounded corners. Its content and controls stay sharp and usable. The screen follows the window width only while it is in the page.
`agent`, `agent_interface` and `artifacts_panel` return the current parts.

## Styles

The page installs the dialog stylesheet with the other built-in element styles
when its `ContributionHost` or standalone `ChatPanel` is created. Every colour, font and radius comes from
the selected brand pack's custom properties. Both light modes paint the panel and
the canvas with one swatch, so a panel fill never marks a state alone: the
highlighted model and the active settings tab carry an accent edge, an idle
filter a border ring, and the proxy switch and its thumb a border edge. The
storage notice is a panel framed in the warning colour. The Close control stays
fully opaque in the muted colour.

## Markdown and reasoning

Markdown and the reasoning disclosure also mount as native page content and as
the `markdown-block` and `thinking-block` elements; see
[native Markdown and code](transcript.md#native-markdown-and-code) and
[page content](frontend.md#page-content).

## Native download and preview/source controls

[`download_button`](../../crates/maestro-web/src/components/download_button/dom.rs)
mounts a native button that saves the caller's original bytes through the one
scoped Blob/anchor owner. `DownloadButtonProps::new` selects a small ghost button
with a small glyph and the `text/plain` MIME type; callers then set the tooltip,
the visible label, the button size, the emphasis, the glyph extent and forced
Base64 decoding. An absent tooltip uses the localized `Download` text and an
explicitly empty one stays empty; `show_text` adds the localized label
independently. Localization is read only where it is used, so a control with an
explicit tooltip and no visible label builds even when the language cannot be
read. The button is a plain action, so it never submits a surrounding
form. Text content qualifies for decoding only at four or more characters, a
multiple of four, with the complete Base64 alphabet and at most two terminal
`=`; a failed decode saves the original text. Byte content keeps the caller's
native array, so a later write or a subview is read when the control activates.
The mounted root owns the activation listener and releases it on drop.

[`PreviewCodeToggle`](../../crates/maestro-web/src/components/preview_code_toggle.rs)
keeps one `ViewMode` in shared owned state. `mode` reads it and `set_mode` writes
it silently; `DomContent::mount` and `update` publish a native group with two icon
buttons, and `render` keeps the retained canvas pair for callers that still draw
on the canvas. A real user change updates the mode, then dispatches one bubbling
`mode-change` event whose detail is `preview` or `code`; selecting the current
mode reports nothing, and a listener that selects again keeps its own choice. The
buttons keep their native form behavior, so neither sets `type`. Registering the
`preview-code-toggle` element gives page authors the same group with an
unreflected `mode` property that reads and writes that enum and repaints the
mounted buttons. An update refreshes the buttons' localized titles and hands the
group to the controller it publishes, so later clicks reach that controller while
the buttons keep their native nodes and focus. Disconnecting
removes the group's handlers, and reconnecting restores them once with the
selection intact.

The `mode-toggle` element shares the same native button owner. Its `modes`
property supplies literal labels in order, and fewer than two labels show no
buttons. `selectedIndex` changes selection silently; `setMode(index)` reports
only a changed, in-range selection through a bubbling `mode-change` event with
`{index, mode}` detail. Selection made by a listener remains selected.

## Tool content

[Tool renderers](tool-renderers.md) provide named dispatch, retained tool headers,
input/output listings and literal console output.

## Operation errors

The native field, the dialogs and the provider and settings components return
`tools::types::ToolUiError`, keeping localization and native failures with their
causes. Shared code, Markdown and console rendering return
`components::code_block::ReadingUiError`; highlighting keeps the original theme
error, and native publication keeps `AttachmentHostError`. Copy buttons and
preview/source toggles return `components::copy_button::LocalizedControlError`
for localization, shared control drawing and selected-color I/O errors.
These Rust source interfaces keep failure kinds and causes with the operation
that owns them, without changing rendering, callback timing or retained lifetimes.

## Custom provider cards

[`CustomProviderCard`](../../crates/maestro-web/src/components/custom_provider_card.rs)
mounts through `DomContent` with the existing `CustomProvider` handle:

```rust
use std::{cell::RefCell, rc::Rc};
use maestro_web::{CustomProvider, CustomProviderCard};

fn provider_card(provider: Rc<RefCell<CustomProvider>>) -> CustomProviderCard {
    let mut card = CustomProviderCard::new(provider);
    card.on_edit = Some(Rc::new(|provider| {
        provider.borrow_mut().name = "Edited provider".into();
    }));
    card
}
```

The card starts in manual mode and displays the supplied model count. Set
`is_auto_discovery` independently of the provider protocol to display the supplied
`components::custom_provider_card::ProviderStatus`; an absent discovery result
omits that row. Names, protocol spellings and URLs remain literal native text.

Call `DomContent::update` after changing operands. Existing buttons keep their
nodes, focus and selection while their actions remain present. Activation reads
the most recently published callback and provider handle, including later edits
to that provider. It releases component borrows before calling the action, so the
action may update its containing card. Removing an action releases its binding;
disconnected roots perform no action, and reconnecting restores delivery without
a second registration. Dropping `DomRoot` releases every retained binding even
when the page still holds its nodes. Localization failures leave the last
successfully published content in place.

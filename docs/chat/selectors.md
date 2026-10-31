# Session search

The `maestro-chat` library exposes session query parsing, matching and ordering
through `components::session_selector_search` and the crate root. These operations
are available independently of the session picker UI.
The [session list](session-selector.md) consumes these operations for its retained
query and membership policies. Its optional named-filter manager defaults to
the [application keybindings](../application/startup.md); scope, sort and path
controls continue to use the active global manager.

Queries combine ordered-character fuzzy terms with quoted contiguous phrases.
Phrases lowercase text and collapse whitespace using the shared text predicate.
An unclosed quote falls back to whitespace-separated fuzzy terms, retaining quote
characters. Only lowercase `re:` selects a case-insensitive regular expression;
expressions receive raw search text. Empty expressions report `Empty regex` in the
parsed query; invalid expressions yield no matching sessions.

Search reads the ID, name, full message text and working directory, separated by
spaces. Missing, null or non-string IDs add no ID text; their records remain
searchable through the remaining fields. Other session metadata is not searched.
Phrase and expression scores use the first match's UTF-16 offset; fuzzy scoring
is owned by the [terminal toolkit](../terminal/completion.md#shared-fuzzy-ranking).

`NameFilter::Named` independently excludes missing or blank names;
`NameFilter::default()` includes named and unnamed sessions. Blank queries preserve
name-filtered input order. For nonblank queries, `SortMode::Recent` keeps incoming
order; `Threaded` and `Relevance` rank by ascending score, then newest modification.
Equal ranks retain input order and duplicate records. Nonblank empty quotes still
reach ordering. Results borrow the original session records.

# Terminal selectors

The crate root and the corresponding `components` modules export
`ThinkingSelectorComponent`, `ThemeSelectorComponent`,
`ShowImagesSelectorComponent` and `UserMessageSelectorComponent`.
Each exposes the existing `Container` child-array editing interface. Render the
outer component, but focus its `get_select_list()` or `get_message_list()` handle.
The focus handle remains usable after clearing or dropping the outer component.

`ThinkingSelectorComponent::new` takes the current `maestro_app::ThinkingLevel`,
the available levels, select/cancel callbacks and a `LiveTheme`. Available levels
keep their supplied order and duplicates; the first current match is selected,
or the first available level when unmatched. Confirmations report typed levels,
including `Off`; an empty list cannot confirm.

`ThemeSelectorComponent::new` takes the current name, select/cancel/preview
callbacks and a `ThemeState`. Discovery errors propagate. Navigation previews
the selected theme; programmatic filtering or index changes do not preview.
The caller owns applying and restoring previews. The list displays at most ten
names, annotating only the current name.

`ShowImagesSelectorComponent::new` takes the current bool, select/cancel callbacks
and a `LiveTheme`. Its Yes/No choices report the confirmed bool.

The message module exposes `UserMessageItem`, `UserMessageList` and
`UserMessageSelectorOptions`. Pass chronological messages, select/cancel callbacks
and options holding the initial id, `LiveTheme` and `TuiRuntime` to
`UserMessageSelectorComponent::new`. An absent, empty or unmatched initial id
selects newest; duplicate matches select the first. Timestamps are accepted but
not displayed. Up/down wrap through history; confirmation reports the entry id.
The returned list's optional callbacks can be replaced or cleared, including
from inside a callback. Input uses the active configurable keybindings.

Message text replaces each LF with a space and trims the shared whitespace
predicate, then uses the terminal toolkit's cell-aware truncation. The window
shows at most ten messages, retaining absolute history positions. Borders and
list colors read the live theme; the message header/help keeps its constructor
styling. The toolkit owns list layout and text wrapping.

Only an empty message constructor schedules cancellation after 100 ms through
the supplied runtime. That one-shot keeps the original cancellation callback;
removing the panel, replacing the list callback or cancelling early does not
suppress it. The callback is released after it runs. Nonempty construction
schedules no cancellation.

## Provider authentication picker

`OAuthSelectorComponent::new` takes an `Arc<maestro_app::AuthStorage>`, ordered
`AuthSelectorProvider` records, select/cancel callbacks and `OAuthSelectorOptions`.
Options supply `OAuthSelectorMode::Login` or `Logout`, a `LiveTheme`, an optional
fallible `get_auth_status` callback and an owned `TuiRuntime`. `AuthType::OAuth` offers subscription authentication;
`AuthType::ApiKey` offers key configuration. The caller supplies and classifies
providers; order and duplicate choices are retained.

Stored credentials show configured status when their kind matches, or a warning
when the stored kind differs. Unstored subscription choices are unconfigured.
Unstored key choices show environment, runtime, fallback, model-file key or
model-file command status from the supplied resolver, or the store's default.
Status labels follow the source even when its `configured` flag is false.
Construction returns `Result` and propagates an initial metadata failure without
returning a picker. Inherent `handle_input` and the trait's `try_handle_input`
return metadata errors at the failing rebuild, retaining selection/search changes
and rows already built. Legacy trait `handle_input` reports errors through the
options runtime; focused writer dispatch instead uses the writer's host, exactly
once, with no post-error redraw. See [fallible input](../terminal/components.md#components).
The picker only reads credentials; callbacks own authentication and logout.
The application root exposes the existing credential records, status, store and
interchangeable memory/backend contracts.

Typing filters names, identities and authentication kinds through the
[shared fuzzy matcher](../terminal/completion.md#shared-fuzzy-ranking). Filtering
retains the selected position when possible and clamps it otherwise. Up/down
clamp rather than wrap. The list shows at most eight choices with an absolute
position count when rows are hidden. Confirm reports the selected provider ID;
cancel reports cancellation. Input uses the current configurable keybindings.

Focus the picker itself: its independent focus flag immediately forwards changes
to the retained search input. The inherited `Container` child array can be edited,
cleared or replaced without discarding selection or the retained input's submission
callback. Direct edits to that child do not filter until input reaches the picker.
Title styling is captured at construction, list styling and visible statuses at
rebuild, and borders read the live theme at render. Render, invalidation and
confirmation alone do not rebuild the list. Text and cursor layout belong to the
[terminal toolkit](../terminal/input.md).
# Model picker

`ModelSelectorComponent::new` takes a `TUI`, shared mutable
`maestro_app::SettingsManager`, mutable `maestro_app::ModelRegistry`,
`ModelSelectorCallbacks` and `ModelSelectorOptions`. Construction refreshes the
registry, propagates refresh failures and requests one render after a completed
load or caught availability failure. Options supply the
current shared `Model`, ordered `ScopedModel` records, initial search and
`LiveTheme`; the application re-exports their existing catalog/settings owners.

The picker starts scoped when scoped records exist, otherwise all. Scoped order,
duplicates and missing models are retained; registry matches use refreshed
handles. Current-model comparisons recognize shared handles without reading their
descriptor; independent descriptors use their live provider and ID.
All models put current provider/ID matches first, followed by stable
locale provider order. Search uses IDs, providers and their combinations, not
model names, with [shared fuzzy ranking](../terminal/completion.md#shared-fuzzy-ranking).
Filtering clamps the existing selection. Switching scope retains the query.

Input reads live configurable bindings: Tab, up, down, confirm and cancel take
precedence in that order. Tab without scoped models is consumed. Navigation wraps;
empty confirmation does nothing. Confirmation updates both default provider and
model before reporting the original shared handle, using its current metadata.
Registry file diagnostics appear alongside usable rows. An availability failure
replaces that diagnostic with its exact message and empties both model views,
while preserving the original scoped or all-provider help. Refresh and locale
failures remain outside this catch. The window shows at most
ten rows, a current-model checkmark and the selected model name unless a diagnostic
is present.

Focus the outer picker; its independent flag forwards to `get_search_input()`.
The returned Input supports submission even after clearing or dropping the outer
Container. Direct Input edits refilter only on subsequent picker dispatch.
The inherited Container child editing interface remains available. Borders use
the live theme, while help and list Text children retain the styling of their last
owning update. The [toolkit](../terminal/components.md) owns wrapping and cursors.

## Session, display and behaviour settings

`SettingsSelectorComponent::new` takes a `SettingsConfig`, `SettingsCallbacks`,
`LiveTheme` and a borrowed `maestro_tui::TerminalImage`. Boolean policies are
grouped into `SessionSettings` (auto-compact, quiet startup and skill commands),
`BehaviorSettings` (hidden thinking, collapsed changelog and install telemetry)
and `TerminalDisplaySettings` (hardware cursor, clear on shrink and terminal
progress). Image display, auto-resize and blocking are direct boolean fields;
image width, editor padding and autocomplete height are supplied `f64` fields.
Other fields use the existing
`maestro_app` setting, transport and thinking types, with supplied available
thinking levels and theme names. Configuration supplies state; the panel does
not persist it.

Render the outer component and route input/focus to `get_settings_list()`.
This returns the same retained `SettingsList` handle, independently of the
`Container` chrome. The searchable main view shows at most ten rows in this
order: auto-compact, show images, image width, auto-resize images, block images,
skill commands, hardware cursor, editor padding, autocomplete max items, clear
on shrink, terminal progress, steering mode, follow-up mode, transport, hidden
thinking, collapsed changelog, quiet startup, install telemetry, double-escape
action, tree filter, warnings, thinking level and theme. Show images and image
width appear only when the supplied terminal supports inline images, giving 23
rows with image support and 21 without. Support is sampled once at construction;
changing capabilities affects new panels, not retained ones. The toolkit owns
[settings search and list rendering](../terminal/components.md).

Numeric initial values keep their supplied value and shortest decimal spelling,
including negative zero displayed as `0` and nonfinite values. Activation cycles
image width through `60`, `80`, `120`, editor padding through `0`–`3`, and
autocomplete height through `3`, `5`, `7`, `10`, `15`, `20`. An unmatched value
advances to the first choice. These fixed decimal choices report `f64` values;
boolean choices report `bool` values.

Cyclic rows report only their typed change callback. Unknown stored delivery,
transport and double-escape spellings remain visible until activation chooses
the first available value. Thinking choices retain supplied order and duplicates;
confirmation notifies before returning to the main view, while cancellation
leaves the value unchanged. Theme navigation optionally previews; cancellation
restores the theme committed when that submenu opened. Confirmation notifies
once before returning, without an extra preview. Empty option lists cannot
confirm but can cancel.

The warnings submenu commits each toggle before notifying with an owned
`WarningSettings` snapshot. Unknown warning members keep their values and order;
missing or null extra-usage preferences display enabled. Closing keeps toggles,
and reopening reads the latest state. Callback invocation holds no warning-state
borrow. List and border colors read the live palette; settings cursors and
submenu heading/help styling are captured when constructed. Navigation,
confirmation and cancellation use live configurable keybindings.
The main settings list also activates on literal Space after navigation takes
precedence; searchable lists accept ordinary text.

## Sign-in dialog

`LoginDialogComponent::new` takes a render request, literal provider id,
completion callback and `LoginDialogOptions`. Pass the toolkit writer's
[`weak_render_request`](../terminal/rendering.md) to avoid retaining the writer.
Options supply the [live theme](../theme.md), platform observation and optional
provider-name/browser effects. Default names come from
`maestro_app::presentation_data::get_oauth_providers`. A nonempty name override
wins; a present title override wins even when empty.

`show_auth` replaces content with linked URL, click hint and optional nonempty
instructions. Native launch uses `open`, `explorer.exe` or `xdg-open`, passing
one literal URL argument through the [shared process owner](../tools-process.md).
Startup and exit failures are silent; the dialog never waits for browser exit.
Browser embeddings provide their own launcher.

`show_manual_input` and `show_prompt` append a row and an input that starts empty;
`show_prompt` also optionally shows an example. Both install an owned
`LoginInputFuture` eagerly, before requesting rendering. Submission returns the
exact text once and freezes the row: the input is replaced by a plain `> value` row, so
earlier answers stay visible and unchanged while a later prompt is active. Dropping an observer does not cancel the dialog; replacing or
dropping its sender settles the old observer as `LoginDialogError::Cancelled`.
Information replaces visible rows, while waiting and progress append. None of
these updates discards a pending input or the text typed into it.

Live cancel bindings win before child input dispatch. Cancellation aborts the
shared signal, rejects pending input with `Login cancelled`, then reports
completion once. Further cancellation can reject a newly installed input without
reporting completion again. Callback reentry is allowed. Theme errors propagate
unchanged; each successful update requests one render.

The dialog retains one input and forwards every focus setter to it, including
unchanged values and while hidden. Inherited child-array edits preserve retained
content/input aliases. New text captures its current styling; borders read the
live theme. Editing, wrapping and cursor geometry remain with the
[terminal components](../terminal/components.md).

## Scoped model editor

`components::ScopedModelsSelectorComponent` edits the supplied cycling scope.
`ModelsConfig` retains shared model descriptors and an optional ordered identity
list: `None` enables all models; `Some(vec![])` is an explicit empty scope.
Explicit order, duplicates and unavailable identities remain in callback payloads
and counts; unavailable identities do not produce rows. Repeated available
identities use the last supplied descriptor while retaining row occurrences.

Install the application's `presentation_data::keybindings::KEYBINDINGS` in the
active key manager. The editor reads live bindings: Enter toggles, Ctrl+A enables,
Ctrl+X clears, Ctrl+P toggles the selected provider, Alt+Up/Alt+Down reorder,
and Ctrl+S requests persistence. A nonempty search restricts enable/clear;
provider toggles still cover the entire provider. All-enabled detection checks
identity coverage rather than list length. Filtered reorder clamps the cursor
at both ends. Up/down wrap within the search results, which match ID and provider,
not display name.

`ModelsCallbacks::on_change` reports edits immediately, after rebuilding the view.
`on_persist` runs only on save; save clears the unsaved marker after the callback's
synchronous invocation. Each receives an owned selection snapshot and may return
an asynchronous tail to the supplied host without delaying further input. Host
failures remain owned by the [terminal runtime](../terminal/rendering.md).

`get_search_input` retains the editable child without installing a submit
callback. Direct child edits refilter on dispatch branches that fully refresh the
view, not navigation or save. Caller-added child render errors propagate through
`try_render`. Parent focus
forwards immediately while the child's own focus flag remains independently
editable. Clearing the inherited Container children removes presentation, not
selection or callback capability. Literal Ctrl+C clears a nonempty query before
cancelling; literal Escape always cancels.

Eight visible rows use the shared centered list window. Borders sample the live
theme; header styling stays captured, navigation rebuilds rows, edits rebuild rows
and footer, and save rebuilds only the footer. Wrapping, cursor decoration and
cell widths belong to the [terminal components](../terminal/components.md).

## Resource configuration

Native callers use
[`components::config_selector::ConfigSelectorComponent`](../../crates/maestro-chat/src/components/config_selector/mod.rs).
Pass `maestro_app::ResolvedPaths`, a shared mutable `SettingsManager`,
`ConfigSelectorOptions` and `ConfigSelectorCallbacks`. Options supply project and
agent directories, a prepared `Collator`, live theme and a live bold-style policy.
Relative patterns resolve against the real process directories, which are separate
from the selected project.
The component does not acquire packages or reload resources.

Render the component, but focus and dispatch input to `get_resource_list()`.
The same list survives inherited `Container` child-array edits. Focus forwards
through that retained list to its search input. Groups capture their labels;
items remain shared mutable `ResourceItem` handles in callback arguments,
filtering, painting and `update_item`. That operation updates its supplied item
first, then the first grouped item with the same current path and kind.

Packages precede top-level sources. Within each origin, scopes order user,
project, temporary, then source names use stable locale order. Kind headings
follow extensions, skills, prompts, themes; item labels use stable locale order.
Labels are basenames, except extensions outside an `extensions` directory use
`parent/basename`, and skills named `SKILL.md` use their parent directory.
A contradictory project/temporary comparison is resolved into this total order.

Typing matches the current name, kind or full path case-insensitively and retains
matching ancestors. A wholly whitespace query resets the view; other queries
retain leading/trailing whitespace. Filtering selects the first match. Up/down
skip headings and clamp; page keys move 15 flat entries before finding an item.
The painted window holds at most 15 flat entries, with resource-only counts.
Colors and bold policy remain live; header hints stay literal `space` and `esc`.

Live binding priority is up, down, page-up, page-down, cancel, literal Ctrl+C,
literal space or confirm, then search. Cancel/exit do not change settings.
A successful toggle edits its raw selected scope, updates the item, then invokes
`on_toggle`; the initial callback requests rendering. Callback slots can be
replaced or cleared, including during synchronous re-entry, without a list/item
borrow held across notification.

Only project resources use project settings; user and temporary resources use
global settings. Top-level patterns are relative to the supplied agent directory
or the project's configuration directory. Package patterns use an explicitly
supplied base (including empty), otherwise the resource's parent directory.
The first matching package source is edited; no match still updates the row and
notifies without writing settings. Exactly matching strings, after stripping one
leading `!`, `+` or `-`, are replaced by one signed pattern. Other entries, package
members and the other scope are retained. Wrong-typed roots/filters read as empty;
unknown members survive through the [settings adapters](../settings.md#reading).
The existing settings owner controls persistence and `flush`/`reload` completion.

Relative paths observe the live process directory only when needed at toggle
time. Inherent `ResourceList::handle_input` and toolkit `try_handle_input`
propagate the original native error before changing rows, settings or
notifications. The required legacy input bridge discards that error without
inventing a fallback path. List and composed `try_render` propagate theme or
child failures; legacy render returns no rows on failure, never a partial frame.
Displayed tabs use the toolkit's [tab expansion](../terminal/text.md#measuring).

# Presented lifecycles

The configuration and session selectors are also run from the command line.
`maestro-cli` owns `select_config` and `select_session`, which take the prepared
options, the frame writer and the factory that builds the chat component, so the
frontend never imports these widgets. The presentation focuses the handle the
component retains, starts the frame writer, and settles exactly once: a close, a
cancellation or a failed presentation stops what it already started, which is
the writer and, for the configuration presentation, the theme watch. The
explicit-exit notification ends the process after those effects. A repeated
notification after the first settles nothing.

The records both fronts hand a component live in
`maestro_app::presentation_data::input_selectors`: the presented handles, the
configuration options and callbacks with their factory, and the session
loaders, request, callbacks and factory. `maestro-chat` re-exports the records
its widgets consume, and `maestro-cli` re-exports `ConfigSelectorOptions` while
importing the rest from that application path, so one declaration states what
each caller supplies. The chat widget keeps its own differently shaped
`ConfigSelectorOptions`, and session discovery keeps its own progress callback.

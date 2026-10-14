# Session search

The `maestro-chat` library exposes session query parsing, matching and ordering
through `components::session_selector_search` and the crate root. These operations
are available independently of the session picker UI.

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
handles. All models put current provider/ID matches first, followed by stable
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

## Session and behaviour settings

`SettingsSelectorComponent::new` takes a `SettingsConfig`, `SettingsCallbacks`
and `LiveTheme`. Boolean policies are grouped into `SessionSettings`
(auto-compact and quiet startup) and `BehaviorSettings` (hidden thinking,
collapsed changelog and install telemetry). Other fields use the existing
`maestro_app` setting, transport and thinking types, with supplied available
thinking levels and theme names. Configuration supplies state; the panel does
not persist it.

Render the outer component and route input/focus to `get_settings_list()`.
This returns the same retained `SettingsList` handle, independently of the
`Container` chrome. The searchable main view shows at most ten rows in this
order: auto-compact, steering mode, follow-up mode, transport, hidden thinking,
collapsed changelog, quiet startup, install telemetry, double-escape action,
tree filter, warnings, thinking level and theme. The toolkit owns
[settings search and list rendering](../terminal/components.md).

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
submenu heading/help styling are captured when constructed. All input follows
live configurable keybindings.

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

`show_manual_input` appends and retains entered text. `show_prompt` appends,
optionally shows an example and resets text. Both install an owned
`LoginInputFuture` eagerly, before requesting rendering. Submission returns the
exact text once. Dropping an observer does not cancel the dialog; replacing or
dropping its sender settles the old observer as `LoginDialogError::Cancelled`.
Information replaces visible rows, while waiting and progress append. None of
these updates discards a pending input or its retained text.

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

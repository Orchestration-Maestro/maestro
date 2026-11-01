# Session-tree navigation

[`TreeList`](../../crates/maestro-chat/src/components/tree_selector/tree_list.rs)
uses the shared [session-tree projection owner](../sessions/tree-projection.md) through
`maestro_app::export_html::template` to retain a presentation of the supplied [session tree records](../sessions/session_format.md).
Its selected-node getter returns a shared occurrence handle; an empty view returns `None`.
Handles and their ordered children remain usable after the list and input containers
are dropped. Distinct occurrences stay distinct even when their records are equal.
Occurrences that share an original entry share one retained immutable record.
Label and timestamp accessors borrow raw optional metadata; cloned handles edit
the same occurrence's slots without affecting another occurrence's metadata.

The list orders active subtrees first at each branch, keeping supplied order
within the active and inactive groups. Repeated entry identities remain separate
navigation positions. Selection chooses the last visible occurrence of an identity.
Hidden-parent lookup uses the last occurrence in the full navigation order.

An omitted filter starts in `Default`. The five modes are:

| Mode | Included entries after assistant eligibility |
|---|---|
| `Default` | Conversation entries, excluding label, custom state, model/thinking changes and session information |
| `NoTools` | Default entries without tool-result messages |
| `UserOnly` | User messages |
| `LabeledOnly` | Entries with a resolved label, including empty or null labels |
| `All` | Entries of any kind |

Assistant entries other than the current leaf need nonblank text or a truthy stop reason other than
`stop` or `toolUse`. The current leaf bypasses this eligibility check, not the
selected mode. Content eligibility uses the shared session owner with the [text](../text.md) whitespace
predicate and [JSON](../json.md) truth predicate. A reached truthy nonstring text
block returns `InvalidData`; an earlier nonblank text block avoids later reads.

Initial selection uses the explicit target when supplied, otherwise the current
leaf. Filtering retains the selected identity, walks hidden parents to a visible
ancestor, or falls back to the last visible row. Hidden ancestry cycles also use
that fallback. Empty results skip parent lookup, have no selection, and retain
the bookmark for a later nonempty filter.

## Configurable input

Callers install a `maestro_tui::KeybindingsManager` built from
[`maestro_app::presentation_data::keybindings::KEYBINDINGS`](../../crates/maestro-app/src/presentation_data/keybindings.rs)
using `maestro_tui::set_keybindings`. The list reads the active manager on input;
construction does not install one. The application table extends toolkit defaults
with seven tree-filter actions, two fold/segment actions, label-edit notification,
label timestamp toggling and three [session display controls](session-selector.md#discovery).

Up/down wrap; page actions and left/right aliases clamp by the supplied visible
line count. Confirm calls `on_select` with the selected entry identity when both
selection and callback exist. Ordinary cancel calls `on_cancel` when supplied,
after a nonempty query has first been cleared without notification.
An empty view cannot confirm a node. Direct filter shortcuts select the requested
mode when it differs from the current mode, or return to default when it is already
selected; the default shortcut always selects default. Forward and
backward cycles follow `Default`, `NoTools`, `UserOnly`, `LabeledOnly`, `All`.
Callbacks are shared callables installed through `set_on_select`, `set_on_cancel`
and `set_on_label_edit`; their getters return the installed callback. Setters replace
or remove it. Notifications release list and metadata borrows before calling out,
so callbacks may reenter the list or replace themselves. The label action reports
the selected identity and its raw optional label; it does not edit that label.

## Search and folds

Printable chunks extend the authored query; any C0, DEL or C1 control rejects the
whole chunk unless a registered action takes precedence. Backspace removes one
Unicode scalar, including a separate combining mark, not one grapheme. Literal
search delegates to the shared session owner, which lowercases the query and
entry text and requires every nonempty token.
Tokens use the shared [text](../text.md) whitespace predicate; search is neither
fuzzy nor regular-expression based.

Search covers attached labels and entry-specific role/text, shell commands,
custom-message kinds/content, branch summaries, titles, model IDs, thinking
levels, custom-state kinds and labels. Compaction contributes its keyword, not
its summary; unknown kinds contribute only their attached label. Message and
custom text-block previews use the shared session prefix helper with a 200-UTF-16-unit
cutoff; custom string content
is not limited. Search and category eligibility intersect.

Typing, nonempty-query Backspace, query-clearing cancel and every filter action
clear folds. Empty-query Backspace and ordinary cancel leave folds intact.
Control/alt-left folds a visible root or segment start with visible children;
otherwise it walks upward to a segment boundary. Control/alt-right unfolds a
folded row; otherwise it follows first children to the next branch or leaf.
Folds hide descendants through filtered intermediate rows; unfolding an ancestor
keeps nested folds. Segment walks stop on repeated semantic identity, retaining
the selected occurrence if no boundary is reachable.

## Rendering

`render(width, &LiveTheme, &dyn TuiRuntime, styles_enabled)` returns the visible
window and position/filter status, clipped with the toolkit's terminal-cell
formatter. Branch gutters follow the filtered parent projection; hidden parents
attach to the nearest visible ancestor. Plain prefixes come from the shared session
owner; fold markers and theme roles remain here. Active marks follow the current leaf,
not the selection. Attached nonempty labels precede entry content.

The list borrows display fields only when their row reaches the window. Reached
malformed text or a missing matched tool-argument receiver returns `InvalidData`;
theme failures are propagated. Tool results resolve the last matching call from
all ordered assistant rows, including hidden rows. Path summaries read the supplied
runtime's nonempty HOME, then USERPROFILE, at each render and shorten a raw prefix.
`command`, `path`, `file_path` and `pattern` use their tool defaults for falsy
values. Before tool-specific formatting, truthy strings supply text and other
truthy values become compact JSON. Shell summaries still normalize newlines and
tabs, trim and truncate; paths still shorten a matching home prefix, and an empty
or `/` home leaves them as written. Numeric read ranges have separate formatting
rules. Entry text remains strictly string-valued. Rendering performs no
filesystem or scheduling operations.

The supplied live theme is read on every render. `styles_enabled` controls bold
selection and italic empty titles; colors remain theme-owned. `invalidate()` is a
no-op because there is no render cache. The retained list has an inherent render
method, not a toolkit `Component` implementation. Folded connector rows use `└⊞ `
or `├⊞ ` in the existing dim prefix role; folded rows without a connector use
`⊞ ` in the accent role. Render releases the list borrow before live host calls.

Message previews use the shared [UTF-16 prefix helper](../text.md) before newline/tab
normalization and shared-whitespace trimming. Tool summaries reuse the application's
JSON and read-range exports; their tool-specific prefix budgets remain here.
## Panel and labels

[`TreeSelectorComponent`](../../crates/maestro-chat/src/components/tree_selector/panel.rs)
composes borders, title, navigation help, the live authored query and the retained
list. `TreeSelectorOptions` supplies selection, filter, terminal height, theme,
runtime, decorations and an optional wall clock. Pages use half the height, at
least five rows. `get_tree_list` returns the same shared list through editing,
container edits and invalidation. Constructor help captures key hints and theme;
search, borders, rows and editor hints remain live. Initially empty roots alone
schedule cancellation after 100 ms through the supplied runtime.

The label shortcut opens a stable `Input` with fresh editing state and cursor
zero. Confirm precedes cancel and ordinary input. Saving trims the shared
whitespace set; an empty result removes the label. The selected entry identity
is captured when editing starts. The row updates before `on_label_change`, and
the editor closes only after normal callback return. A caller panic leaves the
updated row and editor available. Cancel changes no metadata. Focus uses the
shared toolkit forwarding flag to reach the editor's hardware cursor marker.

`update_node_label` edits the first matching navigation occurrence without
rebuilding query, filters or folds. The supplied label is written before the
clock runs. A nonempty label uses the explicit timestamp, including an empty
string, or generates a UTC millisecond timestamp. An empty or absent label
clears the timestamp; a directly supplied empty label remains present.

The registered `app.tree.toggleLabelTimestamp` action defaults to `shift+t`.
It retains the view and appends ` [+label time]` after the filter badge, including
empty views. Timestamp segments require nonempty string labels and timestamps.
The shared [date parser](../sessions/messages.md) interprets dates; unparseable
text suppresses only the segment without modifying metadata. Local same-day
segments use `HH:MM`, same-year segments use `M/D HH:MM`, and other years prefix
the last two characters of the decimal year. Visible valid segments sample the
same supplied clock used for updates; hidden segments do not sample it.

The panel implements toolkit component, input, focus and container-child
interfaces. SDK callers use `try_render` and the inherent `handle_input` to
retain original reached list errors; the writer's infallible trait paths serve
valid data.
Application startup installs the effective manager in [#217](https://github.com/Orchestration-Maestro/maestro/issues/217).

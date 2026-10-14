# Session-tree navigation

[`TreeList`](../../crates/maestro-chat/src/components/tree_selector/tree_list.rs)
retains a presentation of the supplied [session tree records](../sessions/session_format.md).
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
selected mode. Content checks use the shared [text](../text.md) whitespace
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
with seven tree-filter actions, two fold/segment actions and label-edit notification.

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
search lowercases the query and entry text and requires every nonempty token.
Tokens use the shared [text](../text.md) whitespace predicate; search is neither
fuzzy nor regular-expression based.

Search covers attached labels and entry-specific role/text, shell commands,
custom-message kinds/content, branch summaries, titles, model IDs, thinking
levels, custom-state kinds and labels. Compaction contributes its keyword, not
its summary; unknown kinds contribute only their attached label. Message and
custom text-block previews reuse the shared prefix helper; custom string content
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
attach to the nearest visible ancestor. Active marks follow the current leaf,
not the selection. Attached nonempty labels precede entry content.

The list borrows display fields only when their row reaches the window. Reached
malformed text or a missing matched tool-argument receiver returns `InvalidData`;
theme failures are propagated. Tool results resolve the last matching call from
all ordered assistant rows, including hidden rows. Path summaries read the supplied
runtime's nonempty HOME, then USERPROFILE, at each render and shorten a raw prefix.
Tool argument fields use their tool defaults for falsy values. Truthy strings
render unchanged; other truthy values render as compact JSON. Entry text remains
strictly string-valued. Rendering performs no filesystem or scheduling operations.

The supplied live theme is read on every render. `styles_enabled` controls bold
selection and italic empty titles; colors remain theme-owned. `invalidate()` is a
no-op because there is no render cache. The retained list has an inherent render
method, not a toolkit `Component` implementation. Folded connector rows use `└⊞ `
or `├⊞ ` in the existing dim prefix role; folded rows without a connector use
`⊞ ` in the accent role. Render releases the list borrow before live host calls.

Message previews use the shared [UTF-16 prefix helper](../text.md) before newline/tab
normalization and shared-whitespace trimming. Tool summaries reuse the application's
JSON and read-range exports; their tool-specific prefix budgets remain here.
The search display, label editor, timestamp display and selector panel remain in
[#957](https://github.com/Orchestration-Maestro/maestro/issues/957).
Application startup installs the effective manager in [#217](https://github.com/Orchestration-Maestro/maestro/issues/217).

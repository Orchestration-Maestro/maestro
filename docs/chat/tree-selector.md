# Session-tree navigation

[`TreeList`](../../crates/maestro-chat/src/components/tree_selector/tree_list.rs)
borrows the supplied [session tree records](../sessions/session_format.md).
Its selected-node getter returns the original node; an empty view returns `None`.

The list orders active subtrees first at each branch, keeping supplied order
within the active and inactive groups. Repeated entry identities remain separate
navigation positions. Identity lookup chooses the last occurrence.

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
ancestor, or falls back to the last visible row. Empty results have no selection
and retain the bookmark for a later nonempty filter.

## Configurable input

Callers install a `maestro_tui::KeybindingsManager` built from
[`maestro_app::presentation_data::keybindings::KEYBINDINGS`](../../crates/maestro-app/src/presentation_data/keybindings.rs)
using `maestro_tui::set_keybindings`. The list reads the active manager on input;
construction does not install one. The application table extends toolkit defaults
with seven tree-filter actions.

Up/down wrap; page actions and left/right aliases clamp by the supplied visible
line count. Confirm calls `on_select` with the selected entry identity when both
selection and callback exist. Ordinary cancel calls `on_cancel` when supplied.
An empty view cannot confirm a node. Direct filter shortcuts toggle the requested
mode back to default; the default shortcut always selects default. Forward and
backward cycles follow `Default`, `NoTools`, `UserOnly`, `LabeledOnly`, `All`.

Rendering and visual metadata are tracked in [#764](https://github.com/Orchestration-Maestro/maestro/issues/764).
Search, folds, labels and the selector wrapper are tracked in [#768](https://github.com/Orchestration-Maestro/maestro/issues/768).
Application startup installs the effective manager in [#217](https://github.com/Orchestration-Maestro/maestro/issues/217).

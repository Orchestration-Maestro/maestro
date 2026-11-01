# Keyboard shortcuts

Open `/hotkeys` in an interactive session to see keys from your loaded bindings.
Use this reference when editing `keybindings.json`; action IDs are case-sensitive.
The [toolkit API](terminal/keybindings.md) is for embedding applications.
[Terminal key forwarding](terminal/setup.md) covers terminal and tmux setup when
a modified key never reaches Maestro.

## Change a shortcut

The file is `<agent directory>/keybindings.json`, normally
`~/.maestro/agent/keybindings.json`. A nonempty `MAESTRO_CODING_AGENT_DIR`
changes that directory. There is no project-local shortcut file. See
[configuration roots](application/configuration.md#identity-and-roots).

1. Back up an existing file before editing it. Store one JSON object in the file.
2. Set each action to a string, a list of strings, or `[]` to disable it.
3. Run exact `/reload` while the session is idle, then open `/hotkeys`.

For example, this replaces the external-editor shortcut, gives upward movement
two keys, and removes model cycling from Ctrl+P:

```json
{
  "app.editor.external": "alt+e",
  "tui.editor.cursorUp": ["up", "ctrl+p"],
  "app.model.cycleForward": []
}
```

An override replaces the whole default list. Remove its property to restore the
default; `null` is not a disable value. Unknown action IDs do nothing. Duplicate
keys within an action collapse to one key. Key strings are not normalized before
that, so `Ctrl+C` and `ctrl+c` count as two keys that name the same key. Prefer the
spellings in the tables below.

Unreadable files, invalid JSON and non-object roots load defaults without a file
error. Wrong-typed properties (including lists with non-string members) are
ignored. Reloading a missing or malformed file replaces existing overrides with
defaults; it does not keep the previous working bindings. The loader does not
rewrite the file. [Reload](chat/commands.md#reload) also reloads resources and
extensions, clears the draft, and refuses to run during streaming or compaction.

### Reset multiplexer overrides

The native application operation
`presentation_data::keybindings::reset_multiplexer_keybindings(agent_dir, timestamp)`
reads `keybindings.json` from the supplied agent directory. The timestamp is Unix
time in seconds. It creates `keybindings.json.bak-<timestamp>` exclusively and
syncs the exact original bytes before publishing the edited document atomically.
An existing backup is never overwritten.

Only top-level action IDs beginning with `multiplexer.` are removed. Chat and
unknown entries keep their JSON values. The input must be a UTF-8 JSON object;
malformed or non-object input fails without changing the file. Publication
follows existing symlinks and preserves target permissions. A backup or precommit
write failure leaves the original unchanged. A directory-sync warning after
publication is returned separately from failure. Reload or restart the workspace
daemon to use the restored defaults.

The CLI presentation function `workspace_commands::settings::run_reset_keys`
validates the arguments after `settings reset-keys`, calls the application
operation, and prints the backup path and reload guidance. Extra arguments return
2 without file access. Application errors retain their cause and return 1;
output errors propagate to the executable.

### Check collisions

Rebinding a key does not remove it from other actions. For example:

```json
{
  "app.model.select": "ctrl+x",
  "app.tools.expand": "ctrl+x"
}
```

Both actions retain Ctrl+X; this does not mean both run. The focused component's
dispatch order decides which action receives input. Assign different keys
rather than relying on dispatch order. The manager's `get_conflicts()` reports only
keys explicitly assigned to multiple known actions. It does not report collisions
with defaults, equivalent key spellings or terminal-byte aliases.

Some shared defaults are intentional: Ctrl+P cycles models in the editor but
changes the path display in the session picker. Those controls belong to different
focused components. By contrast, assigning Ctrl+P to cursor movement also requires
disabling or moving `app.model.cycleForward`, which intercepts it in the editor.
Extension shortcuts can intercept input before application and editor actions;
see [extension shortcut dispatch](chat/extension-ui.md#shortcuts).

### Emacs-style vertical movement

The editor already has Ctrl+A/E for line boundaries, Ctrl+B/F for character
movement, Alt+B/F for words, and Ctrl+Y for deleted text. Add vertical movement
without losing the default arrow keys:

```json
{
  "tui.editor.cursorUp": ["up", "ctrl+p"],
  "tui.editor.cursorDown": ["down", "ctrl+n"],
  "app.model.cycleForward": []
}
```

Ctrl+N remains a named-session filter in the session picker. This example changes
editor movement, not every selector's controls.

### Modal-style navigation without a mode switch

Use Alt with home-row keys to keep unmodified letters available for typing:

```json
{
  "tui.editor.cursorLeft": ["left", "alt+h"],
  "tui.editor.cursorDown": ["down", "alt+j"],
  "tui.editor.cursorUp": ["up", "alt+k"],
  "tui.editor.cursorRight": ["right", "alt+l"]
}
```

These are alternate navigation keys, not a modal editor. There is no insert/normal
mode switch in this configuration. Lists mean alternative keys, not multi-key
sequences. To change list navigation too, bind `tui.select.up` and
`tui.select.down` separately. Do not bind plain `h`, `j`, `k` or `l` unless you
intend to consume those letters instead of typing them.

## Key notation and terminal limits

Combine the modifiers `ctrl`, `shift`, `alt`, `super` and `hyper` with `+`, for
example `ctrl+shift+x`. Modifier words may come before or after the key: `k+ctrl`
names the same key as `ctrl+k`. The aliases `control`, `option` and `meta` (alt), and
`cmd` and `command` (super), name the same modifiers. A word that is neither a
modifier name nor the one key, such as the typo in `ctlr+k`, makes the whole key string
name nothing, so it never binds the plain key. `hyper+k` binds only a key reported with
hyper held.
The workspace key combinations use the same notation.
Use lowercase letters, digits, literal punctuation, `f1` through `f12`, or named
keys: `escape` (`esc`), `enter` (`return`), `tab`, `space`, `backspace` (`bs`), `delete`,
`insert`, `clear`, `home`, `end`, `pageUp`, `pageDown`, `up`, `down`, `left`, `right`.
The [key decoder reference](terminal/keys.md) gives the supported combinations.

A binding can match only input that reaches Maestro. A terminal, desktop shortcut,
SSH client or multiplexer may consume a key or send the same bytes for different
keys. Changing JSON cannot recover an intercepted key. Try a different key first,
then check the forwarding configuration of each layer. The terminal/tmux setup
guide is tracked in [#2838](https://github.com/Orchestration-Maestro/maestro/issues/2838).

Legacy input cannot always distinguish Ctrl+I from Tab, Ctrl+M from Enter or
Ctrl+[ from Escape. Shift+Enter needs a distinguishable terminal sequence;
linefeed matches Enter without the enhanced protocol but Shift+Enter with it.
Enhanced keyboard reporting improves modifier information only when the terminal
and forwarding layers preserve it. See [legacy ambiguities](terminal/keys.md#legacy-ambiguities)
and [native terminal negotiation](terminal/native-terminal.md).

## Default bindings

Each table lists full IDs for use as JSON properties. Commas separate alternatives;
`none` means `[]`. The tables describe registered actions, not keys active in every
component. `/hotkeys` shows common editing/application controls and extension keys,
not this entire action inventory.

### Writing and moving through text

| Action ID | Default keys | Use |
| --- | --- | --- |
| `tui.editor.cursorUp` | `up` | Previous visual row; history when applicable |
| `tui.editor.cursorDown` | `down` | Next visual row; history when applicable |
| `tui.editor.cursorLeft` | `left`, `ctrl+b` | Previous character |
| `tui.editor.cursorRight` | `right`, `ctrl+f` | Next character |
| `tui.editor.cursorWordLeft` | `alt+left`, `ctrl+left`, `alt+b` | Previous word boundary |
| `tui.editor.cursorWordRight` | `alt+right`, `ctrl+right`, `alt+f` | Next word boundary |
| `tui.editor.cursorLineStart` | `home`, `ctrl+a` | Beginning of the line |
| `tui.editor.cursorLineEnd` | `end`, `ctrl+e` | End of the line |
| `tui.editor.jumpForward` | `ctrl+]` | Find a character ahead |
| `tui.editor.jumpBackward` | `ctrl+alt+]` | Find a character behind |
| `tui.editor.pageUp` | `pageUp` | Move a page toward earlier text |
| `tui.editor.pageDown` | `pageDown` | Move a page toward later text |
| `tui.editor.deleteCharBackward` | `backspace` | Remove the preceding character |
| `tui.editor.deleteCharForward` | `delete`, `ctrl+d` | Remove the following character |
| `tui.editor.deleteWordBackward` | `ctrl+w`, `alt+backspace` | Remove text to the previous word boundary |
| `tui.editor.deleteWordForward` | `alt+d`, `alt+delete` | Remove text to the next word boundary |
| `tui.editor.deleteToLineStart` | `ctrl+u` | Remove text back to the line boundary |
| `tui.editor.deleteToLineEnd` | `ctrl+k` | Remove text ahead to the line boundary |
| `tui.editor.yank` | `ctrl+y` | Insert the latest kill-ring text |
| `tui.editor.yankPop` | `alt+y` | Replace a yank with an older kill-ring entry |
| `tui.editor.undo` | `ctrl+-` | Restore the preceding edit snapshot |
| `tui.input.newLine` | `shift+enter` | Add a line without submitting |
| `tui.input.submit` | `enter` | Accept input |
| `tui.input.tab` | `tab` | Completion or the component's Tab action |
| `tui.input.copy` | `ctrl+c` | Reserved copy action; consumed by the toolkit editor |

The multiline editor uses deleted-text history for yank/rotation; this is separate
from the system clipboard. See [editor behavior](terminal/editor.md). The registered
copy action does not perform a system-clipboard copy in the toolkit editor; the
chat editor uses Ctrl+C for `app.clear`.

### Choosing a list item

| Action ID | Default keys | Use |
| --- | --- | --- |
| `tui.select.up` | `up` | Previous item |
| `tui.select.down` | `down` | Next item |
| `tui.select.pageUp` | `pageUp` | Earlier page, where supported |
| `tui.select.pageDown` | `pageDown` | Later page, where supported |
| `tui.select.confirm` | `enter` | Accept the highlighted item |
| `tui.select.cancel` | `escape`, `ctrl+c` | Leave the selection or clear its search first |

Components differ in wrapping, paging and search behavior. See
[selectors](chat/selectors.md) and [session-tree input](chat/tree-selector.md#configurable-input).

### Controlling the interactive session

| Action ID | Default keys | Use |
| --- | --- | --- |
| `app.interrupt` | `escape` | Dismiss completion or interrupt current work |
| `app.clear` | `ctrl+c` | Empty the draft; a rapid second press exits |
| `app.exit` | `ctrl+d` | Exit with an empty draft |
| `app.suspend` | `ctrl+z` (none on native Windows) | Return to the shell temporarily |
| `app.editor.external` | `ctrl+g` | Edit the draft using `VISUAL` or `EDITOR` |
| `app.clipboard.pasteImage` | `ctrl+v` (`alt+v` on native Windows) | Attach a clipboard image |
| `app.tools.expand` | `ctrl+o` | Switch tool-output detail |
| `app.thinking.toggle` | `ctrl+t` | Switch thinking-block visibility |
| `app.model.select` | `ctrl+l` | Choose a model |
| `app.model.cycleForward` | `ctrl+p` | Next model in the cycle |
| `app.model.cycleBackward` | `shift+ctrl+p` | Previous model in the cycle |
| `app.thinking.cycle` | `shift+tab` | Next thinking level |
| `app.message.followUp` | `alt+enter` | Queue input after the current run |
| `app.message.dequeue` | `alt+up` | Bring queued input back into the draft |

A second clear press exits only when it arrives less than 500 ms after the
previous clear. Ctrl+D with text in the draft remains forward deletion.
For image tools, external-editor requirements and queue behavior, read
[interactive mode](chat/interactive-mode.md), especially
[queued input](chat/interactive-mode.md#queued-input).

### Opening and managing saved sessions

| Action ID | Default keys | Use |
| --- | --- | --- |
| `app.session.new` | none | Start a fresh session; also `/new` |
| `app.session.tree` | none | Browse branches; also `/tree` |
| `app.session.fork` | none | Choose a fork point; also `/fork` |
| `app.session.resume` | none | Choose a saved session; also `/resume` |
| `app.session.togglePath` | `ctrl+p` | Change path visibility in session views |
| `app.session.toggleSort` | `ctrl+s` | Change session ordering |
| `app.session.toggleNamedFilter` | `ctrl+n` | Restrict the picker to named sessions |
| `app.session.rename` | `ctrl+r` | Rename the selected session |
| `app.session.delete` | `ctrl+d` | Request deletion of the selected session |
| `app.session.deleteNoninvasive` | `ctrl+backspace` | Request deletion only with an empty search |

The last six controls act inside session views, not as global editor commands.
Deletion has a confirmation step; see [session picker](chat/session-selector.md).

### Browsing branches

Open `/tree` to use these controls. See [tree search and folds](chat/tree-selector.md#search-and-folds).

| Action ID | Default keys | Use |
| --- | --- | --- |
| `app.tree.foldOrUp` | `ctrl+left`, `alt+left` | Fold a segment or go to an earlier boundary |
| `app.tree.unfoldOrDown` | `ctrl+right`, `alt+right` | Unfold a segment or follow the branch |
| `app.tree.editLabel` | `shift+l` | Change the selected node's label |
| `app.tree.toggleLabelTimestamp` | `shift+t` | Change label timestamp visibility |
| `app.tree.filter.default` | `ctrl+d` | Restore the conversation view |
| `app.tree.filter.noTools` | `ctrl+t` | Switch the tool-result filter |
| `app.tree.filter.userOnly` | `ctrl+u` | Switch to/from user-message filtering |
| `app.tree.filter.labeledOnly` | `ctrl+l` | Switch to/from labeled-entry filtering |
| `app.tree.filter.all` | `ctrl+a` | Switch to/from the complete entry view |
| `app.tree.filter.cycleForward` | `ctrl+o` | Next filter |
| `app.tree.filter.cycleBackward` | `shift+ctrl+o` | Previous filter |

### Arranging the model cycle

Open `/scoped-models`. Changes apply to the session immediately; saving writes
`enabledModels`. See [model scope editing](chat/commands.md#editing-the-model-scope).

| Action ID | Default keys | Use |
| --- | --- | --- |
| `app.models.save` | `ctrl+s` | Persist the selection without closing the panel |
| `app.models.enableAll` | `ctrl+a` | Enable the search matches, or every model without a query |
| `app.models.clearAll` | `ctrl+x` | Disable the search matches, or every model without a query |
| `app.models.toggleProvider` | `ctrl+p` | Change the highlighted provider's enabled state |
| `app.models.reorderUp` | `alt+up` | Put the highlighted model earlier in the cycle |
| `app.models.reorderDown` | `alt+down` | Put the highlighted model later in the cycle |

## Suspension on Unix, WSL and Windows

On Unix, including WSL, press the bound suspend key (normally Ctrl+Z) to stop the
process group and restore the shell's terminal. Run `fg` in that shell to resume
and redraw. See the
[suspend lifecycle](chat/interactive-mode.md#suspend).

Native Windows has no default suspend key. A manually assigned `app.suspend`
shows `Suspend to background is not supported on Windows`; it does not emulate
Unix job control. The registered Windows defaults above do not imply that the
executable can open interactive mode there: native Windows startup is blocked by
[#2844](https://github.com/Orchestration-Maestro/maestro/issues/2844).
WSL runs the Unix path and is not that native-Windows case.

## Key names in hints

Every hint spells its keys from the live registry through `key_text`, `key_display_text` and
`format_key_text`, and none writes a key by hand. `key_text` joins the keys bound to an action
with `/` as written, with `alt` shown as `option` on macOS. `key_display_text` also starts each
part with a capital letter (`Ctrl+Option+X`), and `format_key_text(key, capitalize)` applies the
same spelling to a key list that is not a registry action, such as an extension shortcut. The
settings list, the hotkey list, the startup help and the model-scope line printed before the
interactive mode starts all use them, so a remapped key shows as configured. The model-scope
line names the key bound to the next-model action and leaves its hint out when none is bound.


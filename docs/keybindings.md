# Keyboard shortcuts

Open `/hotkeys` in an interactive session to see the keys your loaded bindings
define. Use this page when you edit `keybindings.json`; action IDs are
case-sensitive. The [toolkit API](terminal/keybindings.md) serves embedding
applications. When a modified key never reaches Maestro, read
[terminal key forwarding](terminal/setup.md) for terminal and tmux setup.

## Change a shortcut

The file is `<agent directory>/keybindings.json`, normally
`~/.maestro/agent/keybindings.json`. A nonempty `MAESTRO_CODING_AGENT_DIR`
changes that directory. Shortcuts have no project-local file. See
[configuration roots](application/configuration.md#identity-and-roots).

1. Back up an existing file before you edit it. The file holds one JSON object.
2. Set each action to a string, a list of strings, or `[]` to disable it.
3. Run the exact command `/reload` while the session is idle, then open `/hotkeys`.

This example replaces the external-editor shortcut, gives upward movement two
keys, and frees Ctrl+P from model cycling:

```json
{
  "app.editor.external": "alt+e",
  "tui.editor.cursorUp": ["up", "ctrl+p"],
  "app.model.cycleForward": []
}
```

An override replaces the whole default list. Remove the property to restore the
default; `null` does not disable an action. Unknown action IDs do nothing.
Duplicate keys within an action collapse to one key. Maestro does not normalize key
strings first, so `Ctrl+C` and `ctrl+c` count as two keys that name the same key.
Use the spellings in the tables below.

An unreadable file, invalid JSON or a non-object root loads the defaults without a
file error. Maestro ignores wrong-typed properties, including lists with non-string
members. Reloading a missing or malformed file replaces existing overrides with the
defaults; the previous working bindings do not survive. The loader never rewrites
the file. [Reload](chat/commands.md#reload) also reloads resources and extensions,
clears the draft, and refuses to run during streaming or compaction.

### Reset multiplexer overrides

The native application operation
`presentation_data::bindings::reset_multiplexer_keybindings(agent_dir, timestamp)`
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

Rebinding a key does not remove it from other actions. In this example:

```json
{
  "app.model.select": "ctrl+x",
  "app.tools.expand": "ctrl+x"
}
```

Both actions keep Ctrl+X, but only one receives the input. The focused component's
dispatch order decides which. Assign different keys instead of relying on that
order. The manager's `get_conflicts()` reports only keys explicitly assigned to
several known actions. It does not report collisions with defaults, equivalent key
spellings or terminal-byte aliases.

Some shared defaults are deliberate: Ctrl+P cycles models in the editor and changes
the path display in the session picker, because the two controls belong to different
focused components. Assigning Ctrl+P to cursor movement is different: you must also
disable or move `app.model.cycleForward`, which intercepts the key in the editor.
Extension shortcuts can intercept input before application and editor actions;
see [extension shortcut dispatch](chat/extension-ui.md#shortcuts).

### Emacs-style vertical movement

The editor already binds Ctrl+A/E to line boundaries, Ctrl+B/F to character
movement, Alt+B/F to words and Ctrl+Y to deleted text. This adds vertical movement
and keeps the default arrow keys:

```json
{
  "tui.editor.cursorUp": ["up", "ctrl+p"],
  "tui.editor.cursorDown": ["down", "ctrl+n"],
  "app.model.cycleForward": []
}
```

Ctrl+N stays the named-session filter in the session picker. The example changes
editor movement only, not the controls of every selector.

### Modal-style navigation without a mode switch

Alt with home-row keys leaves the unmodified letters free for typing:

```json
{
  "tui.editor.cursorLeft": ["left", "alt+h"],
  "tui.editor.cursorDown": ["down", "alt+j"],
  "tui.editor.cursorUp": ["up", "alt+k"],
  "tui.editor.cursorRight": ["right", "alt+l"]
}
```

These are alternate navigation keys, not a modal editor: the configuration has no
insert/normal mode switch. A list holds alternative keys, never a multi-key
sequence. To change list navigation too, bind `tui.select.up` and
`tui.select.down` separately. Bind plain `h`, `j`, `k` or `l` only when you want
those letters consumed instead of typed.

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

A binding matches only input that reaches Maestro. A terminal, desktop shortcut,
SSH client or multiplexer may consume a key, or send the same bytes for different
keys. Editing the JSON cannot recover an intercepted key. Try a different key
first, then check the forwarding configuration of each layer. The terminal/tmux setup
guide is tracked in [#2838](https://github.com/Orchestration-Maestro/maestro/issues/2838).

Legacy input cannot always distinguish Ctrl+I from Tab, Ctrl+M from Enter or
Ctrl+[ from Escape. Shift+Enter needs a distinguishable terminal sequence;
linefeed matches Enter without the enhanced protocol but Shift+Enter with it.
Enhanced keyboard reporting improves modifier information only when the terminal
and forwarding layers preserve it. See [legacy ambiguities](terminal/keys.md#legacy-ambiguities)
and [native terminal negotiation](terminal/native-terminal.md).

## Default bindings

Each table lists the full IDs to use as JSON properties. Commas separate
alternatives, and `none` means `[]`. The tables describe registered actions, not
keys that every component honors. `/hotkeys` shows the common editing and
application controls and the extension keys, not this whole action inventory.

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

The multiline editor keeps a deleted-text history for yank and rotation, separate
from the system clipboard. See [editor behavior](terminal/editor.md). The registered
copy action does not copy to the system clipboard in the toolkit editor; the chat
editor uses Ctrl+C for `app.clear`.

### Choosing a list item

| Action ID | Default keys | Use |
| --- | --- | --- |
| `tui.select.up` | `up` | Previous item |
| `tui.select.down` | `down` | Next item |
| `tui.select.pageUp` | `pageUp` | Earlier page, where supported |
| `tui.select.pageDown` | `pageDown` | Later page, where supported |
| `tui.select.confirm` | `enter` | Accept the highlighted item |
| `tui.select.cancel` | `escape`, `ctrl+c` | Leave the selection or clear its search first |

Wrapping, paging and search differ between components. See
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

The last six controls act inside session views and are not global editor commands.
Deletion asks for confirmation; see [session picker](chat/session-selector.md).

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

On Unix, including WSL, the bound suspend key (normally Ctrl+Z) stops the process
group and restores the shell's terminal. Run `fg` in that shell to resume and
redraw. See the [suspend lifecycle](chat/interactive-mode.md#suspend).

Native Windows has no default suspend key. A manually assigned `app.suspend` shows
`Windows has no suspend to background` and does not emulate Unix job
control. The registered Windows defaults above do not imply that the
executable can open interactive mode there: native Windows startup is blocked by
[#2844](https://github.com/Orchestration-Maestro/maestro/issues/2844).
WSL runs the Unix path and is not that native-Windows case.

## Key names in hints

On macOS, a binding such as `alt+x` appears as `option+x` in a hint or `Option+X`
in `/hotkeys`. Keep `alt+x` in `keybindings.json`; the display name does not change
which key triggers the action. The longer label can wrap a hint or widen a table
column.

Every hint spells its keys from the live registry through `key_text`, `key_display_text` and
`format_key_text`; no hint writes a key by hand. `key_text` joins the keys bound to an action
with `/` as written. `key_display_text` also starts each part with a capital letter
(`Ctrl+Option+X`), and `format_key_text(key, capitalize)` applies the
same spelling to a key list that is not a registry action, such as an extension shortcut.
`plain_key_hint` uses the same platform spelling without theme colors. The
settings list, the hotkey list, the startup help and the model-scope line printed before the
interactive mode starts all use them, so a remapped key shows as configured. The model-scope
line names the key bound to the next-model action and leaves its hint out when none is bound.


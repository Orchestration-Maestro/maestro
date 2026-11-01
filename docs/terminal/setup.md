# Terminal key forwarding

Use this guide when Shift+Enter submits instead of adding a line, Alt+Enter
never reaches Maestro, or an input method opens its candidate window away from
the editor. Check the terminal first, then tmux, then Maestro's bindings. A
binding cannot recover modifiers that the terminal discarded.

1. [Inspect received keys](#inspect-received-keys)
2. [Configure your terminal](#configure-your-terminal)
3. [Forward keys through tmux](#forward-keys-through-tmux)
4. [Place the hardware cursor](#place-the-hardware-cursor)
5. [Diagnose remaining failures](#diagnose-remaining-failures)

## Inspect received keys

Maestro's native adapter queries the Kitty keyboard protocol at startup, followed by the
terminal's device attributes. When the attributes arrive and no Kitty reply came first, it
requests xterm `modifyOtherKeys` mode 2; no timer decides. A Kitty reply that arrives later
turns that mode off and the Kitty protocol on.
Maestro decodes both enhanced CSI-u reports and xterm reports. The
[native terminal reference](native-terminal.md#keyboard-protocols) describes
negotiation; the [key decoder](keys.md) describes accepted input.

The default interactive actions are:

| Press | Action | Result |
| --- | --- | --- |
| Enter | `tui.input.submit` | Submit input; during a response, queue steering |
| Shift+Enter | `tui.input.newLine` | Insert a line into the draft |
| Alt+Enter | `app.message.followUp` | Queue a follow-up during a response; submit when idle |
| Ctrl+Enter | No default action | Available for a custom binding if the terminal forwards it |

See [queued input](../chat/interactive-mode.md#queued-input) for delivery and
compaction behavior, and [keyboard shortcuts](../keybindings.md) for overrides.

To inspect bytes without contacting a model:

1. Open a disposable Unix terminal, outside tmux, with at least 40 rows so the
   tester's input log and hints fit on screen. From a source checkout with
   [development tools installed](../development.md#setup-and-build), run:

   ```sh
   cargo run --locked -p maestro-tui-crossterm --example key_tester
   ```

2. Press Enter, Shift+Enter, Ctrl+Enter and Alt+Enter separately. The tester
   displays `Hex`, `Chars` and `Repr`; it does not display action names. A terminal
   or desktop shortcut may consume a chord before the tester receives it.
3. Compare the observations below, then press Ctrl+C to exit. Repeat inside
   tmux. If only the tmux result loses modifiers, use the tmux section below.

These are representative encodings, not a requirement that every terminal send
the same bytes. `\x1b` denotes ESC; `\r` denotes carriage return.

| Input | CSI-u | xterm `modifyOtherKeys` |
| --- | --- | --- |
| Shift+Enter | `\x1b[13;2u` | `\x1b[27;2;13~` |
| Alt+Enter | `\x1b[13;3u` | `\x1b[27;3;13~` |
| Ctrl+Enter | `\x1b[13;5u` | `\x1b[27;5;13~` |
| Ctrl+C | `\x1b[99;5u` | `\x1b[27;5;99~` |

Plain Enter commonly arrives as `\r`. If Shift+Enter also arrives as `\r`,
Maestro cannot distinguish those presses. A literal linefeed (`\n`) is also
ambiguous: it matches Ctrl+J; without the enhanced protocol it also matches
Enter, while with that protocol active it matches Shift+Enter. Prefer forwarding
the explicit modified-key report rather than remapping Shift+Enter to linefeed.
See [legacy ambiguities](keys.md#legacy-ambiguities).

## Configure your terminal

Change only the relevant entries in an existing configuration. Back it up first;
do not replace the file with a fragment below. Restart or reload the terminal
as its own documentation requires, then repeat the key tester.

### Kitty and iTerm2

Start with the current defaults and test before adding mappings. Maestro requests
enhanced reporting itself. In iTerm2, check whether the active profile's Option
key or a shortcut intercepts the chord you need. Consult the terminal's
[keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) or
[iTerm2 key settings](https://iterm2.com/documentation-preferences-profiles-keys.html)
when changing profile behavior. Do not assume a profile copied from another
machine still forwards the same bytes.

### Ghostty

Use Ghostty's **Open Configuration** action to locate the active configuration.
If Alt+Backspace does not arrive as a word-deletion key, this binding sends
ESC followed by DEL:

```text
keybind = alt+backspace=text:\x1b\x7f
```

Look for a `shift+enter=text:\n` override. It replaces the modified chord with
a linefeed, so remove that override if you want an explicit Shift+Enter report.
Maestro's multiline editor already inserts a newline for raw linefeed, but it
cannot tell that remapped chord apart from physical Ctrl+J. An explicit report
keeps the key distinguishable for other bindings. Ghostty's
[keybinding reference](https://ghostty.org/docs/config/keybind/reference)
owns the terminal-side syntax.

### WezTerm

Enable Kitty keyboard reporting in the active Lua configuration. For a new
`~/.wezterm.lua`, the complete configuration is:

```lua
return {
  enable_kitty_keyboard = true,
}
```

For an existing configuration, add this field to its returned table, or assign
`config.enable_kitty_keyboard = true` before returning the existing `config`. See
[`enable_kitty_keyboard`](https://wezterm.org/config/lua/config/enable_kitty_keyboard.html).

### Visual Studio Code integrated terminal

Open **Preferences: Open Keyboard Shortcuts (JSON)** in the command palette.
Add these entries to its array, preserving existing shortcuts:

```json
[
  {
    "key": "shift+enter",
    "command": "workbench.action.terminal.sendSequence",
    "args": { "text": "\u001b[13;2u" },
    "when": "terminalFocus"
  },
  {
    "key": "alt+enter",
    "command": "workbench.action.terminal.sendSequence",
    "args": { "text": "\u001b[13;3u" },
    "when": "terminalFocus"
  }
]
```

These send explicit reports only while the terminal has focus. Check for
conflicting IDE shortcuts if either chord still does not reach the tester. The
[integrated-terminal guide](https://code.visualstudio.com/docs/terminal/advanced#_custom-sequence-keyboard-shortcuts)
describes `sendSequence`.

### Windows Terminal

Open **Settings**, then **Open JSON file**. Merge these objects into the existing
`actions` array:

```json
{
  "actions": [
    {
      "command": { "action": "sendInput", "input": "\u001b[13;2u" },
      "keys": "shift+enter"
    },
    {
      "command": { "action": "sendInput", "input": "\u001b[13;3u" },
      "keys": "alt+enter"
    }
  ]
}
```

Alt+Enter normally toggles fullscreen in Windows Terminal. Assigning it to
`sendInput` gives that chord to the terminal application instead. Reopen Windows
Terminal if an old binding remains active. See Microsoft's
[`sendInput` documentation](https://learn.microsoft.com/en-us/windows/terminal/customize-settings/actions#send-input).

Use WSL for Maestro's Unix interactive executable and the key tester. These
mappings do not enable native Windows interactive startup: that runtime gap is
tracked in [#2844](https://github.com/Orchestration-Maestro/maestro/issues/2844).
Native Windows/macOS runtime test qualification remains tracked in
[#486](https://github.com/Orchestration-Maestro/maestro/issues/486).
The library's [Windows console support](native-terminal.md#windows-consoles)
is separate from executable startup; the shipped terminal demonstrations still
return Unsupported on non-Unix platforms.

### Older terminals and IDE terminal engines

In xfce4-terminal, Terminator and IntelliJ IDEA's integrated terminal, verify
modified Enter rather than assuming the active engine/profile supports it. If
the tester receives ordinary Enter for both chords, changing Maestro's submit
binding to `ctrl+enter` will not help. Use a distinguishable chord or a terminal
with enhanced-key support. Alacritty is another option when its installed
version supports the Kitty keyboard protocol; check its version and test the
actual build rather than relying on the terminal name.

## Forward keys through tmux

Extended-key support starts with tmux 3.2. The selectable output format
`extended-keys-format` requires tmux 3.5 or later. Check the installed version:

```sh
tmux -V
```

Test on a separate server before touching your normal configuration. The
following POSIX-shell sequence creates a new configuration file and a private
socket; it never connects to your default tmux server. Keep this shell open for
cleanup:

```sh
keycheck=$(mktemp -d)
cat > "$keycheck/tmux.conf" <<'EOF'
set -s extended-keys on
EOF
tmux -S "$keycheck/socket" -f "$keycheck/tmux.conf" new-session -d -s keys -x 80 -y 40
tmux -S "$keycheck/socket" show-options -s extended-keys
```

The last command should report `extended-keys on`. Probe the output-format
option; older versions report `invalid option: extended-keys-format`:

```sh
tmux -S "$keycheck/socket" show-options -s extended-keys-format
```

On tmux 3.5 or later, select CSI-u for this private server:

```sh
tmux -S "$keycheck/socket" set-option -s extended-keys-format csi-u
```

On 3.2 through 3.4, omit that command; those releases do not expose a format
selector (3.4 forwards modified Enter in CSI-u form). On 3.5 or later the format
option defaults to `xterm`; choosing `csi-u` changes the representation, not the
Maestro action. Both encodings in the table above work. With `extended-keys on`,
tmux forwards modified keys when the application requests them, as Maestro's
fallback does; it cannot supply modifiers that the outer terminal never sent.

Attach from the disposable terminal with at least 40 rows. tmux follows the
attached client's dimensions, so a shorter client can hide the tester's log:

```sh
tmux -S "$keycheck/socket" attach-session -t keys
```

Inside the private session, inspect the attached-terminal capabilities:

```sh
tmux display-message -p '#{client_termname} #{client_termfeatures}'
```

Then run the key tester from your checkout and compare with the outside-tmux
run. Detach with the default tmux prefix Ctrl+B, then D. Look for `extkeys`
in the capability output. If tmux
does not recognize a terminal that you have verified supports extended keys,
add a narrowly matched terminal feature to the test configuration, for example
`set -as terminal-features ',xterm-kitty:extkeys'` for an actual `xterm-kitty`
client. Recreate the private server or reattach after terminal-feature changes;
do not force `*:extkeys` for every client. See the
[tmux extended-key guide](https://github.com/tmux/tmux/wiki/Modifier-Keys).

When finished, stop only the private server and remove its leftover socket and
configuration:

```sh
tmux -S "$keycheck/socket" kill-server
rm -f "$keycheck/socket" "$keycheck/tmux.conf"
rmdir "$keycheck"
```

After successful testing, merge `set -s extended-keys on` into your normal tmux
configuration (usually `~/.tmux.conf`). On 3.5 or later also add
`set -s extended-keys-format csi-u`. A fresh server reads those settings at
startup. Do not kill a server holding work to apply a keyboard change; use the
private test or arrange a restart after saving your sessions.

## Place the hardware cursor

If an IME candidate window is misplaced, enable Maestro's terminal cursor at the
editor position. Merge this property into your
[settings file](../sessions/settings.md#scopes-and-merging):

```json
{
  "showHardwareCursor": true
}
```

Restart the session after editing settings. With no boolean setting, the default
is hidden unless `MAESTRO_HARDWARE_CURSOR` is exactly `1`. A boolean setting
wins over that environment variable. For example, in a POSIX shell with the
[source executable shortcut](../first-session.md#1-build-from-source) defined:

```sh
MAESTRO_HARDWARE_CURSOR=1 maestro
```

The toolkit positions the hardware cursor at the editor's cursor marker; enabling
it can help an IME anchor its candidate window, but terminal captures cannot
prove physical IME placement. Test composition with your actual input method.
This also applies to IntelliJ IDEA when the terminal hides the hardware cursor.
See [cursor rendering](rendering.md#defaults) for marker and visibility rules.

## Diagnose remaining failures

| Observation | Next check |
| --- | --- |
| Both Enter chords have the same bytes outside tmux | Remove raw-linefeed remaps or fix terminal/IDE key interception |
| Outside works, inside loses modifiers | Check tmux version, `extended-keys`, terminal `extkeys` capability and output format |
| Tester receives distinct reports but Maestro uses the wrong action | Inspect `/hotkeys`, then the action IDs and collisions in `keybindings.json`; reload while idle |
| Alt+Enter changes fullscreen or opens an IDE action | Replace that terminal/IDE shortcut with explicit forwarding |
| Candidate window is misplaced | Enable `showHardwareCursor`, restart, then test with the physical IME |

A key-tester run needs no account or model. If a separate interactive check needs
a response, use a controlled provider and disposable project/configuration;
forwarding tests do not require live keys or paid requests.

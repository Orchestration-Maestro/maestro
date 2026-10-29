# Modal editor

Each session start replaces the host's editor with one built on the application's
custom editor. It starts in insert mode, where all input reaches the editor.
Escape switches to normal mode; a second Escape reaches the editor, which
interrupts the agent. In normal mode `h`, `j`, `k` and `l` move left,
down, up and right, `0` and `$` go to the start and end of the line, and `x`
deletes the character under the cursor. `i` returns to insert mode, and `a`
moves right first. Control keys such as Ctrl+C and Ctrl+D, and other input
longer than one character, pass on in both modes; normal mode ignores any other
single character. The bottom line ends with ` INSERT ` or ` NORMAL `, truncated
to make room, unless that line is narrower than the label.

Build with `--example modal_editor` using the [component build command](README.md). The controlled
`modal_editor_` runs type into the built component on a host custom editor with
supplied key and width answers; they need no provider, credentials or
installation. Loading is opt-in and needs a component-enabled interactive host
that accepts a replacement editor; the example replaces that host's editor in
every session.

## Controlled runs

```sh
capped mise exec -- cargo nextest run --locked -p maestro-extensions-wasm --lib \
  -E 'test(modal_editor_example::)'
```

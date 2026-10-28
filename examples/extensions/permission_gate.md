# Permission gate

`tool_call` leaves non-bash tools and commands outside its three patterns alone.
For case-insensitive matches of `rm` followed by `-r`, `-rf` or `--recursive`,
`sudo` as a word, or `chmod`/`chown` followed on the same line by `777`, it asks
`Allow?` with `Yes` and `No` choices. Only `Yes` permits the call; dismissal or
any other answer blocks it with `Blocked by user`. Without UI it blocks with
`Dangerous command blocked (no UI for confirmation)`.

This is an illustrative regex matcher, not a security sandbox or a shell parser.
It can match quoted text and miss dangerous commands; its flag alternatives also
match prefixes such as `-rubbish`. The host still owns execution. Loading the
component is opt-in and needs a component-enabled host; interactive approval
needs that host's UI. The controlled runs deliver calls to both the in-process
adapter and the built component with scripted selections. They execute no shell
commands and need no provider, credentials or installation. Build it with the
[build command](README.md#building-an-example) using `--example permission_gate`.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- permission_gate_
```

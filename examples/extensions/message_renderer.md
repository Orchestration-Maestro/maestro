# Message renderer

`/status <message>` sends a displayed `status-update` custom message at the
`info` level. A first word of `warn` or `error` sets the level instead, and the
remaining words, joined by single spaces, become the message; with nothing after
the level the message reads `Status report`. The message details hold the level
and the send time in milliseconds since the Unix epoch.

The renderer registered for `status-update` draws `[LEVEL]` in the theme's error,
warning or success color, followed by the message text, inside a padded box over
the theme's custom message background. A missing or non-text level draws as
`[INFO]`. Expanded, a message with a nonzero timestamp adds a dimmed second line,
`sent <time>`, with the host's local time in the en-US twelve-hour form.

Build with `--example message_renderer` using the [build command](README.md#building-an-example). The
controlled `message_renderer_` runs send messages from the built component with a
supplied wall clock, then draw each delivered message through its registered
renderer with supplied theme, widget and local time answers. They need no
provider, credentials or installation. Loading the component is opt-in and needs
an application with the component runtime and an interactive transcript.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- message_renderer_
```

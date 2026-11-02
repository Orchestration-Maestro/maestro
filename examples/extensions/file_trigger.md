# File trigger

Create `/tmp/agent-trigger.txt` before loading the component into a native host
with filesystem and watch support. On each session start the example watches
that fixed path and, when UI is available, reports `Watching /tmp/agent-trigger.txt`.
To send a prompt from another process, write it to the existing file:

```sh
printf '%s\n' 'Run the tests' > /tmp/agent-trigger.txt
```

A change reads and trims the contents. Nonempty text becomes a displayed
`file-trigger` custom message prefixed with `External trigger: `, with
`triggerTurn: true`; the file is cleared only after the host accepts the message.
Empty or unreadable contents do nothing. A missing file at startup fails watch
creation, so create it first. Each new session replaces the previous watch;
shutdown releases it through the existing watch owner.

Loading this example is opt-in. Anyone able to write the trigger file can ask the
agent to act; use it only in a trusted local environment. A real response uses the
host's configured model and can incur provider charges. The controlled runs need
no credentials or paid service: the component run uses a loopback provider. Tests
create the fixed path exclusively and refuse to touch a file already there. The
nextest file-trigger group runs those tests one at a time; do not run separate
file-trigger test commands concurrently.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- file_trigger_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- file_trigger_
```

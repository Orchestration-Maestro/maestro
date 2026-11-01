# Handoff

`/handoff <goal>` selects the current branch's conversation. After compaction it
uses the latest summary, the kept prefix and subsequent messages. Message and
compaction entries are included; settings, labels and other entry kinds are not.
It asks the selected model for a focused prompt, shows a cancellable loader, and
opens **Edit handoff prompt** for review.
Only response text enters the draft, with a newline between text blocks.

Accepting the editor opens a new session with the current session file as its
parent. The replacement context puts the edited prompt in that session's editor
and reports `Handoff ready. Submit when ready.` It does not submit the prompt.
Cancelling generation or the editor leaves the session alone; a host veto of
replacement reports `New session cancelled` without writing to another editor.
Missing credentials or generation failures report `Cancelled`; the generation
error is also written to the host's error output.

Build with `--example handoff` using the [build command](README.md#building-an-example). Loading it is opt-in and
requires a component-enabled interactive host with custom UI, editor dialogs,
event tails and session replacement. Real generation sends the selected history
and goal to the configured model and requires that provider's API key; it can
incur charges. The controlled `handoff_` run uses an in-memory provider, history,
credentials and terminal, with no network calls or extension installation.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- handoff_
```

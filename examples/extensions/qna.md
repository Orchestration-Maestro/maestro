# Qna

`/qna` reads the latest complete assistant reply on the current branch. It skips
non-text blocks and joins text blocks with newlines, without trimming them. An
incomplete newest assistant reply is rejected rather than replaced with an older
one. A reply without text blocks is skipped; a reply with one empty text block
reports that no assistant messages were found.

The command replaces the editor with a cancellable bordered loader while the
selected model extracts questions. Escape cancels the view. Missing credentials,
completion failures and aborted responses report `Cancelled` and leave the editor
unchanged. A completed extraction replaces the editor text exactly as returned,
so the user can fill in answers before submitting. Closing the view disposes the
loader and releases its pending extraction.

Build with `--example qna` using the [build command](README.md#building-an-example). Loading is opt-in and needs a
component-enabled interactive host, a selected model and its configured API key.
A real extraction sends the assistant reply to that provider and can incur
charges. The controlled `qna_` runs use a loopback provider and supplied terminal
input; they require no credentials, paid service or installation.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- qna_
```

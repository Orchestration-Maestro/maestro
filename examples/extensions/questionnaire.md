# Questionnaire

The `questionnaire` tool asks one or more questions supplied by the caller. A
single question submits as soon as the user selects an option or writes an
answer. Multiple questions use tabs: Tab and the arrow keys switch questions,
Up/Down select an option, and Enter confirms. The Submit tab stays open until
every question has an answer. Answers can be revised without changing their
original insertion order.

Each question offers `Type something.` unless `allowOther` is false. That choice
opens the host's multiline editor. Enter saves the answer after trimming the
shared whitespace set (including U+FEFF, excluding U+0085); a blank answer becomes
`(no response)`. Escape in the editor discards its text and returns to the
options. Elsewhere Escape cancels
the questionnaire and keeps any partial answers in the result details. A
question with no options and `allowOther: false` cannot be answered; Escape still
cancels it. The tool returns an error result without UI or without questions.

Build with `--example questionnaire` using the [build command](README.md#building-an-example). The controlled
`questionnaire_` runs execute the in-process adapter and built component with
supplied terminal input, theme replies and text-helper replies. They compare
frames and complete results
with recorded reference executions. They use no provider, credentials, real
terminal or installation. Loading the component is opt-in and needs a
component-enabled interactive host with custom UI, editor and native text/key
helpers. The example stores no session entries and writes no files.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- questionnaire_
```

# Question

Build with `--example question` using the [build command](README.md#building-an-example). Loading is opt-in and
requires a component host with interactive custom views and retained editor
widgets. The `question` tool takes a question and options with labels and optional
descriptions. Arrow keys navigate; Enter selects a choice or opens “Type
something.” for a custom answer. Empty custom submissions return to the choices.
Escape in the editor clears its text and returns to the choices; Escape there
cancels. Without UI, the tool returns an error message and a null answer.

Result details retain the question, original option labels, answer and, when
answered, whether it was custom. Call rows show numbered choices, including
custom input; result rows distinguish selections, written answers and cancellation.
The `question_` controlled runs exercise both the native callbacks and built
component with scripted terminal input. They need no provider, credentials,
external process or filesystem writes beyond component-build artifacts.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- question_
```

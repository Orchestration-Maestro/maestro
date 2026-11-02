# Structured output

`structured_output` accepts a `headline`, a `summary` and an `actionItems` array
of strings. It returns `Structured answer ready: <headline>` as text, keeps the
three fields in result details and sets the terminate flag. The agent ends the
run on that result without requesting another completion. Both collapsed and
expanded rows show the full themed headline, summary and numbered action items.
Missing, null, false, zero or empty-string details show the first content block
if that block is text.

Build with `--example structured_output` using the [build command](README.md#building-an-example). Loading is
opt-in and needs a component-enabled host; themed rows also need its tool
renderer and text widget support. Asking a real model to call the tool needs
that model's configured credentials and can incur provider charges. The
controlled runs supply one tool-call response and theme replies, require no
provider credentials or live services, and assert one completion request.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- structured_output_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- structured_output_
```

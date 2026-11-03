# Custom footer

`/footer` switches between the host's default footer and one dimmed line showing
input tokens, output tokens, total cost, the current model and the Git branch.
Each render totals the assistant messages on the active session branch; it ignores
other messages and entries. Counts below 1,000 are shown as numbers, larger counts
as one decimal followed by `k`, and cost as dollars with three decimal places.
Without a model the footer shows `no-model`; without a Git branch it omits the
parenthesized branch. The host's terminal helpers measure and clip the line to
its supplied width, preserving styling and whole graphemes.

Branch changes request a render. Restoring the default footer disposes the custom
one and unsubscribes its branch listener. The controlled run invokes the built
component with supplied history, model, branch, theme and terminal-helper replies;
it needs no provider, repository, credentials or installation. Loading the
component is opt-in and needs an application with the component runtime and an
interactive UI host. Build it with the [build command](README.md#building-an-example) using `custom_footer` as
the example name.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- custom_footer_
```

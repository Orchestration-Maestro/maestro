# GitHub issue autocomplete

On session start, `github_issue_autocomplete` resolves the first GitHub SSH or
HTTP remote in `git remote -v` order. It preloads up to 100 open issues with
`gh issue list` in a host-owned listener tail, so startup does not wait for
GitHub. Both commands use the session directory and a five-second timeout.
Every completion request in that session shares the same result, including a
failure; command and JSON failures show one error notification and leave
completion to the previous provider.

Type `#` at the start of a line or after a space or tab. The example offers at
most 20 issues, preferring number prefixes for numeric queries and otherwise
using the host's fuzzy ranking over issue numbers and titles. Choosing an item
uses the previous provider's completion operation; unrelated input and missing
matches also pass through to that provider. The cache refreshes on session start,
not while typing.

Build with `--example github_issue_autocomplete` using the [build command](README.md#building-an-example).
Loading it is opt-in and needs a component-enabled interactive host with editor
completion, `git` and `gh` on its process PATH, a GitHub repository checkout, and
GitHub CLI authentication with permission to read that repository's issues.
Loading runs the two read-only commands and can contact GitHub. The controlled
`github_issue_autocomplete_` tests supply command results and editor requests;
they need no network, credentials, provider or installation.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- github_issue_autocomplete_
```

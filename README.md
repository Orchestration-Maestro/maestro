<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/readme/hero-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/readme/hero-light.svg">
    <img src="assets/readme/hero-light.svg" width="100%" alt="Maestro. Conduct every model. An agent engine in Rust, shown beside its model catalog drawn as a ring of 970 dots around the Maestro compass mark.">
  </picture>
</h1>

<p align="center"><sub>The ring is the offline model catalog on <code>main</code>: one dot per model, 970 from 31 providers.<br>Each sector is one of the nine wire protocols, numbered with its model count. Ember and copper dots are reasoning models.</sub></p>

<p align="center">
  <a href="https://github.com/Orchestration-Maestro/maestro/actions?query=branch%3Amain"><img src="assets/readme/badges/checks.svg" height="24" alt="checks: view runs"></a>
  <a href="rust-toolchain.toml"><img src="assets/readme/badges/rust.svg" height="24" alt="rust: 1.98.0"></a>
  <a href=".github/ci.toml"><img src="assets/readme/badges/platforms.svg" height="24" alt="tested on: Linux"></a>
  <a href="LICENSE"><img src="assets/readme/badges/license.svg" height="24" alt="licence: MIT"></a>
  <a href="docs"><img src="assets/readme/badges/docs.svg" height="24" alt="docs: guides"></a>
</p>

Maestro is an agent engine written in Rust. It sends a conversation to a model,
runs the tools the model asks for, and keeps the history as a tree you can branch
and resume. Every model in its catalog sits behind the same request interface,
whichever protocol its provider speaks.

## Highlights

- One request interface for 970 models from 31 providers, through nine protocol
  adapters [registered side by side](crates/maestro-models/src/builtins/register_builtins.rs).
  The [catalog](docs/models/catalog.md) is compiled in, so listing models makes no
  network call.
- An [agent loop](docs/loop.md) that streams the reply, runs the model's tool calls
  (in parallel by default) and returns the results until the model stops asking.
- Seven [tools](docs/tools.md) to start from: read, write, edit, bash, grep, find
  and ls.
- [Sessions](docs/sessions/session_manager.md) stored as JSONL, one entry per line,
  each pointing to its parent. Open, continue, fork or branch, and the other
  branches stay in the file.
- A [terminal toolkit](docs/terminal/rendering.md) that rewrites only the rows that
  changed and never takes over your scrollback, with an editor, completion,
  Markdown and inline images.
- Lints set to `forbid`: no `unsafe`, no `unwrap`, no panics in production code,
  and documentation on every item, private ones included.

## See it run

<p align="center">
  <img src="assets/readme/chat-demo.svg" width="100%" alt="A terminal running the chat example: the slash-command menu opens, Tab completion lists crates/maestro-tui-crossterm and crates/maestro-tui, a message with a bold word is sent, a Thinking indicator spins and a reply appears.">
</p>

<p align="center"><sub>The terminal toolkit's <a href="docs/terminal/examples.md">chat example</a>, recorded on Linux from <code>main</code>. Replies are simulated locally, so it needs no account or key.</sub></p>

## Quick start

On Linux, with Git and [rustup](https://rustup.rs) installed:

```sh
git clone https://github.com/Orchestration-Maestro/maestro.git
cd maestro
rustup toolchain install    # the Rust version pinned in rust-toolchain.toml
cargo run --locked -p maestro-tui-crossterm --example chat_simple
```

Type `/` for commands, press Tab to complete a path and Ctrl+C to quit.

To work on Maestro itself, install [mise](https://mise.jdx.dev), then run
`mise install` and `mise exec -- just setup` for the pinned tools and commit hooks.
[Development](docs/development.md) covers the rest.

## How it is built

`main` holds 38 crates, and each has one job. Dependencies point one way, from the
front ends down to small leaf crates. A [conventions test](crates/maestro-test-conventions)
reads `cargo metadata` and fails on any edge the
[crate graph](docs/specs/maestro-port.md#crates-and-delivery-order) does not allow.
The rest of the bar is in the build files:

```toml
# Cargo.toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
pedantic = { level = "forbid", priority = -1 }
missing_docs_in_private_items = "forbid"
unwrap_used = "forbid"
expect_used = "forbid"
panic = "forbid"

# clippy.toml
too-many-arguments-threshold = 5
too-many-lines-threshold = 60
cognitive-complexity-threshold = 15
```

Each pull request goes through the same shared CI: formatting, Clippy,
documentation, the tests on Linux, a WebAssembly build and compile checks for
macOS and Windows.

## Status

Maestro has no release yet. The pieces below are libraries; there is no `maestro`
command to install.

| State | Parts |
| --- | --- |
| Built | Models and providers, the agent loop, tools, sessions, settings, credentials, packages and the terminal toolkit, with tests that run on Linux in CI |
| In progress | The application layer, the terminal chat and browser front ends, the Markdown parser |
| Planned | The `maestro` command, the RPC and command-line front ends, the extension runtime, the workspace daemon |

## Documentation

- [Agent loop](docs/loop.md), [tools](docs/tools.md) and [sessions](docs/sessions/session_manager.md)
- [Model catalog](docs/models/catalog.md) and [model selection](docs/selection.md)
- [Terminal toolkit](docs/terminal/components.md) and its [examples](docs/terminal/examples.md)
- [Architecture checks](docs/architecture.md) and the [foundation specification](docs/specs/maestro-port.md)
- [Identity and brand](docs/identity.md)

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening an issue or a pull request.
[AGENTS.md](AGENTS.md) holds the code, documentation and Git rules every change
follows.

## Licence

[MIT](LICENSE).

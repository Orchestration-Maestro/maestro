# Extension Examples

Example extensions for Maestro. The
[examples tracking issue](https://github.com/Orchestration-Maestro/maestro/issues/2074)
lists each one with its owner.

## Usage

An extension crate is a `cdylib` whose root names its factory. The
[extension guide](../../docs/extensions/guest.md) describes the interface the
factory registers.

Maestro discovers extensions in two directories, in this order:

1. `<cwd>/.maestro/extensions`
2. `<agent_dir>/extensions`

[Extension loading](../../docs/extensions/loading.md) describes the remaining
steps.

## Examples

| File | Registers |
| --- | --- |
| `hello.rs` | A `hello` tool that greets the supplied name. |
| `shutdown_command.rs` | A `/quit` command and two tools that request a clean shutdown. |
| `prompt_customizer.rs` | Tool and skill guidance added to the supplied system prompt. |
| `hidden_thinking_label.rs` | A `thinking-label` command that sets the label shown when thinking blocks are hidden, or resets it. |
| [`with-deps/`](with-deps/) | A `parse_duration` tool that parses a human-readable duration to milliseconds with a regular-expression dependency. |
| `reload_runtime.rs` | A `reload-runtime` command that reloads extensions, skills, prompts and themes, and a `reload_runtime` tool that queues that command as a follow-up message. |
| `protected_paths.rs` | A `tool_call` hook that blocks writes and edits whose path contains one of three illustrative substrings, and warns when the host has UI. |
| `pirate.rs` | A `/pirate` command that toggles a pirate response style in the supplied prompt. |
| `trigger_compact.rs` | A `turn_end` hook that requests compaction above 100,000 tokens and a `trigger-compact` command that requests compaction immediately. |
| `bookmark.rs` | A `/bookmark` command that labels the last assistant message, so the tree view can find it again. |

### Building an example

Authors need the pinned toolchain with the `wasm32-wasip2` target. Each example
builds to a component library; there is no command-line entry point:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example <name> --target wasm32-wasip2 --release
```

### Running an example

The controlled runs instantiate the built component in the test host and call
the command and tools it registered:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hello_component_greets_through_the_host
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- shutdown_component_runs_every_registration
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- prompt_guidance
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- hidden_thinking_label_component_runs_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- reload_runtime_component_uses_registered_callbacks
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- protected_paths_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- pirate_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- trigger_compact_component_matches_host_requests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- bookmark_
```

No provider, paid service or extension installation is required. Loading a
built component into an application is opt-in and needs that application's
component runtime.

### What the shutdown example does

`/quit`, `finish_and_exit` and `deploy_and_exit` ask the host to shut down
through their context, and each one keeps the answer it returns. The deployment
tool is illustrative: it reports progress, returns the environment it was given
and starts no deployment process. Loading the component into an interactive host
is opt-in and requests that host's shutdown.

### What the prompt guidance example does

`before_agent_start` returns a replacement `systemPrompt` built from the supplied
options: the prompt the host assembled, guidance for each active built-in tool in
read, bash, edit and write order, the supplied skill names in their original
order, and this extension's own context section. It discovers no resources and
builds no prompt.

The supplied prompt already holds the user append and any edit an earlier handler
made, so the example never appends that option again. Loading the component into
an application is opt-in.

### What the protected-paths hook does

`tool_call` inspects only the `write` and `edit` tools and reads their `path`
argument without normalizing it. A path containing `.env`, `.git/` or
`node_modules/` is blocked with `Path "<path>" is protected`; when the host has
UI, the hook first warns `Blocked write to protected path: <path>` with warning
severity. Every other tool and path is left alone and returns no verdict.
Matching is a case-sensitive literal substring test: it does not resolve dot
segments, fold case, or look at the filesystem. The three patterns are this
example's own policy, not a built-in restriction; loading the component into an
application is opt-in.

The example needs a component-enabled host to load. The controlled run needs no
provider, credentials or installation: it delivers every recorded tool call to
the built component and to the in-process adapter.

### What the hidden thinking label example does

`/thinking-label <text>` sets the collapsed-thinking label the host shows, and
`/thinking-label` without arguments clears it and restores `Pondering...` for the
next session start. The example also applies the label it retains at every
`session_start`, so the next session shows the label the session before set.

A command that sets a label trims the surrounding whitespace and keeps the interior
text as it is. When the host rejects a request the command ends with the host's
message, and the label the example retains stays the one the command chose.
Loading the component into an application is opt-in and needs a host that enables
the component runtime.

### Building with-deps

The [`with-deps/main.rs`](with-deps/main.rs) example parses a duration string
with a regular expression. The dependency is the workspace pin, declared once as
a development dependency of this crate, so it reaches the crate's examples and
tests without entering the guest library; only the pinned toolchain and the
`wasm32-wasip2` target are needed:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example with_deps --target wasm32-wasip2 --release
```

### Running with-deps

The controlled run instantiates the built component in the test host and calls
the registered tool with every recorded duration text:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib with_deps_component_uses_its_parsing_dependency
```

No provider, paid service or extension installation is required. Loading the
built component into an application is opt-in and needs that application's
component runtime.

### What the compaction trigger example does

`turn_end` records the context usage the host reports and requests compaction when
a known estimate at or below 100,000 tokens is followed by a known estimate above
it. `/trigger-compact` requests compaction immediately, and its arguments become
the instructions the host passes on, trimmed, or none when they are blank.

Usage the host does not report leaves the last known estimate as it was, and the
command never changes it. Compaction, its notifications and any history change
belong to the host: the extension only asks for them, reports `Compaction started`
before the request and `Compaction completed` or `Compaction failed: <message>`
when the host answers, each one only while a UI is available.

No provider, paid service or extension installation is required for the
controlled run. Loading the built component into an application is opt-in, and a
real compaction also needs that application's model and runtime configuration.

### What the runtime reload command does

`/reload-runtime` awaits the reload request its command context supplies and
returns nothing else. The `reload_runtime` tool runs with the ordinary context,
which has no reload method, so it queues `/reload-runtime` as a follow-up user
message and answers that it queued it. The controlled run reloads nothing in a
running application; loading the component is opt-in and needs that
application's component runtime.

### What the pirate example does

`/pirate` starts disabled. Each invocation reports the state it switched to and
ignores its arguments. While the mode is on, `before_agent_start` returns the
supplied prompt followed by this example's own pirate instructions; while it is
off, the handler returns no override and leaves the event as it arrived. The
state belongs to the factory instance, so two loaded copies toggle apart, and a
host that refuses the notification still keeps the switch it made first. Loading
the component into an application is opt-in and needs that application's
component runtime.

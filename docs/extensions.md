# Extensions

Extensions add tools, commands and event handlers to Maestro. They run as
WebAssembly components, not as source files or native shared libraries. The
application loads a built component and supplies the extension API and operating
system interfaces it imports. Installing an extension never compiles Rust.

Use an extension to change how a session works: register a tool the model can
call, inspect or block tool calls, change the assembled prompt, manage session
history, add a provider, or offer dialogs and terminal components. The
[working examples](../examples/extensions/README.md) show the patterns available in the shipped examples. Start with
[`hello.rs`](../examples/extensions/hello.rs), which registers one greeting tool.
For terminal views, follow [component authoring and recipes](extensions/components.md)
to connect host widgets, input, focus and cleanup.

Extensions run with the user's access. They inherit environment variables and
standard streams, can use the network, and can read and write through
preopened filesystem roots: `/` on Unix, and each accessible drive and the
working directory's share on Windows. A component is **not an untrusted-code sandbox** in
Maestro. Inspect an extension before loading it; see the
[operating environment](extensions/host_io.md) for the precise access and current
platform limitations.

## Contents

1. [Create the project](#create-the-project)
2. [Build the component](#build-the-component)
3. [Package the component](#package-the-component)
4. [Load and reload](#load-and-reload)
5. [Author workflows](extensions/workflows.md)
6. [Capability reference](#capability-reference)

## Create the project

The supported authoring route today is Rust with `maestro-extensions-wasm`.
Use a source checkout of the same Maestro revision as the host you will run;
these instructions do not assume the library has been published to a registry.
Keep that checkout beside your extension project:

```text
work/
├── maestro/                 # SDK, canonical WIT and hello.rs
└── hello-extension/
    ├── Cargo.toml           # Rust dependencies and component library
    ├── Cargo.lock           # committed dependency resolution
    ├── rust-toolchain.toml  # copied from the matching Maestro checkout
    ├── src/lib.rs           # factory and tool implementation
    ├── justfile             # build recipe below
    ├── package.json         # distribution manifest, added before packaging
    └── dist/index.wasm      # built component, added before packaging
```

From the directory that should contain both projects, obtain Maestro if it is
not already there. Select the revision matching your installation before copying
its toolchain or source:

```sh
git clone https://github.com/Orchestration-Maestro/maestro.git maestro
mkdir hello-extension
cd hello-extension
mkdir src
cp ../maestro/rust-toolchain.toml .
cp ../maestro/examples/extensions/hello.rs src/lib.rs
```

Create `Cargo.toml` with the following contents. The relative dependency path
assumes the layout above; adjust it if your SDK checkout is elsewhere.
`[workspace]` keeps this project independent even when its parent is another
Cargo workspace.

```toml
[package]
name = "hello-extension"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
maestro-extensions-wasm = { path = "../maestro/crates/maestro-extensions-wasm" }
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = { version = "=1.0.151", features = ["float_roundtrip"] }

[workspace]
```

The copied source defines a factory with this signature:

```rust
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()>
```

The factory registers the `hello` tool. Its schema requires a string `name`;
execution returns `Hello, <name>!` and structured details containing `greeted`.
The final `export_extension!(factory)` invocation supplies the component entry
point. Keep its `wasm32` guard so native checks do not export guest bindings.
There is no `main` function, and the `.wasm` file is not a command to execute in
a shell. The [guest authoring guide](extensions/guest.md) explains the factory,
tool definition, callbacks and JSON records.

### WIT world and language support

The SDK owns the interface; do not copy or modify WIT for an ordinary Rust
extension. Its canonical
[`wit/world.wit`](../crates/maestro-extensions-wasm/wit/world.wit) declares
`maestro:extension@0.1.0`, world `extension`, which imports `host` and exports
`guest`. The adjacent `host.wit`, `guest.wit`, `session.wit` and `standard/`
inputs define the capabilities and standard interfaces. Rust's export macro
and SDK bindings implement that world for the factory you provide.

The extension interface uses **WASI 0.3 native asynchronous components**. Rust is
the supported language today. The build target is named
`wasm32-wasip2`: Rust's standard library still imports WASI 0.2 basics, while the
SDK supplies the native asynchronous component bindings and WASI 0.3 operations.
The host supports both generations in one context. A generic WASI 0.2 command or
component does not, by itself, implement Maestro's extension world.

Another language can be added when its toolchain emits WASI 0.3 components with
native async support. Generate bindings from these same canonical WIT inputs,
implement the `guest` exports and consume the `host` imports, then prove factory
registration, callbacks, async waits and resource release against the native
host. This uses the existing component loader, not a second extension system.
JavaScript, TypeScript and Python authoring are not verified routes today;
source-file discovery does not mean those files can execute. There is no polling
adapter or source-language interpreter fallback. See
[native execution](extensions/native-execution.md) and
[standard interfaces](extensions/host_io.md).

## Build the component

Install Rust through rustup. Enter the extension directory so rustup selects the
copied toolchain. Generate the lockfile once, then keep it with your source:

```sh
rustup target add wasm32-wasip2
cargo generate-lockfile
cargo build --locked --target wasm32-wasip2 --release
```

The output is `target/wasm32-wasip2/release/hello_extension.wasm`.
The crate name's hyphen becomes an underscore in the library file name. No
separate component conversion command is needed. If you use Just, put the same
build command in `justfile`:

```just
build:
    cargo build --locked --target wasm32-wasip2 --release
```

Rebuild whenever source or dependencies change. Update `Cargo.lock`
intentionally when changing dependencies; do not remove `--locked` to hide an
unexpected resolution change. Commit the toolchain, manifest, lockfile and
source together. Ignore `target/`; distribute the copied component rather than
that build directory.

For comparison, from the Maestro checkout you can build its unchanged greeting
example with the registered example target:

```sh
~/.local/bin/capped mise exec -- cargo build --locked \
  -p maestro-extensions-wasm --example hello --target wasm32-wasip2 --release
```

That output is under `target/wasm32-wasip2/release/examples/hello.wasm`.
It proves the workspace example, not your separate project's manifest.
The [example instructions](../examples/extensions/README.md) also list the
controlled host tests, which need no provider or credentials.

## Package the component

Packages contain **already-built** components. From your extension project,
copy the release output into the distribution directory:

```sh
mkdir -p dist
cp target/wasm32-wasip2/release/hello_extension.wasm dist/index.wasm
```

Create `package.json`. The npm name below is an example: choose a name and scope
you own before publishing.

```json
{
  "name": "@your-team/maestro-hello",
  "version": "0.1.0",
  "files": ["dist/index.wasm"],
  "maestro": {
    "extensions": ["./dist/index.wasm"]
  }
}
```

`maestro.extensions` lists entry components relative to the package root. Keep
helper files out of that list; each entry creates its own extension instance.
No package-level JavaScript entry point, Rust source compilation or install-time
build script is required. Dependencies linked by Rust are already in the
component. Extra runtime data must also be included in the package, and an
extension can find its artifact through `MAESTRO_EXTENSION_PATH`; see
[native execution](extensions/native-execution.md).

For npm, inspect `npm pack --dry-run` and confirm it includes both `package.json`
and `dist/index.wasm` before publishing through your normal npm release process.
Consumers install the published package with:

```sh
maestro install npm:@your-team/maestro-hello@0.1.0
```

For Git distribution, commit `package.json` and the built `dist/index.wasm` in
the repository and tag the tested revision. A consumer can install that revision:

```sh
maestro install git:github.com/your-team/maestro-hello#v0.1.0
```

If your ignore rules exclude `dist/`, explicitly include the artifact before
committing. A consumer must not need Cargo to use either package. Git package
preparation can run npm dependency installation when a `package.json` is
present, so do not add lifecycle scripts that compile the component.
See [package acquisition and entry rules](packages.md) and
[source syntax and pinning](package-sources.md) for scope, settings, updates and
resource filtering. Use `--local` on `maestro install` for project-scope
installation rather than user-scope installation.

## Load and reload

For the first local run, pass the built artifact directly:

```sh
maestro --no-extensions -e ./dist/index.wasm
```

`--no-extensions` disables automatic discovery; explicit `-e` entries still
load. The executable supplies the component runtime. An SDK embedding must
supply its module loader; see [SDK resource setup](application/sdk.md) and
[native component execution](extensions/native-execution.md).

To enable project discovery instead, copy the component into the project you
want to use it in:

```sh
mkdir -p .maestro/extensions/hello
cp dist/index.wasm .maestro/extensions/hello/index.wasm
maestro
```

The `index.wasm` name is a conventional entry point. An extension directory can
instead declare its entries in `package.json`. Discovery starts with
`<cwd>/.maestro/extensions`, then `<agent_dir>/extensions`, followed by configured
paths. The default agent directory is `~/.maestro/agent`; configuration can change
it. Package installation records its own source in settings. See
[path loading and discovery](extensions/loading.md) and
[application resources](application/resources.md).

The greeting registers a **model-callable tool**, not a `/hello` slash command.
Ask the model to call `hello` with `name` set to `Ada`; the expected text is
`Hello, Ada!`. This interactive call requires your normal model configuration
and can incur provider usage. The automated clean-folder check described below
invokes the same registered tool without a model.

After changing an extension, rebuild it and replace the artifact at the path
Maestro loaded. In interactive mode, run `/reload`. Reload recreates the
extension runtime and reloads skills, prompts and themes; it does not compile
source or copy a new artifact into place. Restart the process when you want a
fresh application run, or when your embedding has no reload command.
Command handlers can await `ExtensionCommandContext::reload`; an ordinary tool
context cannot reload directly. Return immediately after a successful reload
instead of continuing work with the old command context. See
[session replacement and reload](extensions/lifecycle.md) and the
[`reload_runtime` example](../examples/extensions/reload_runtime.rs).

### Check the recipe without a model

From a Maestro checkout, run:

```sh
~/.local/bin/capped mise exec -- cargo test --locked \
  -p maestro-extensions-wasmtime --test integration \
  extension_guide_builds_and_loads_from_a_clean_folder -- --nocapture
```

This check creates a disposable project outside the Cargo workspace, uses the
manifest above and its committed lockfile, copies only `hello.rs` as its source,
and builds the component. It installs that result as a project `index.wasm`,
discovers it through the extension loader and runs its registered tool through
the native artifact runtime. It asserts both `Hello, Ada!` and the structured
`greeted` details. Build dependencies can be cached; the project folder itself
starts empty. No extension package is downloaded or published, and no provider
request is made.

## Author workflows

After loading your first component, use the [author workflows](extensions/workflows.md)
to choose event handlers, wrap built-in tools, bound output, restore branch-local
state and add UI that also handles headless use. Each workflow points to a shipped
example and its owning capability reference. Keep using the build and packaging
recipe above.

## Capability reference

Start with the guest-facing pages when writing a component. Native facade pages
also describe the host policies those calls reach; they are not alternative
native-extension authoring instructions.

| Need | Read |
| --- | --- |
| Factory, imports, tools, commands, flags and shortcuts | [Guest authoring](extensions/guest.md), [registration](extensions/registration.md) |
| Event names, payloads and returned decisions | [Guest events and results](extensions/guest.md#events-and-results), [hook dispatch](extensions/hooks.md) |
| Context, session readers, command-only operations and replacement callbacks | [Guest contexts](extensions/guest.md#contexts-and-sessions), [session lifecycle](extensions/lifecycle.md) |
| Messages, tool selection, model selection, thinking and session entries | [Guest actions](extensions/guest.md#ordinary-actions), [action bindings](extensions/registration.md#actions-and-provider-binding) |
| Persistence across restarts | [Session entries](extensions/guest.md#session-entries), [entry records](entry.md) |
| Event channels between extensions | [Event bus](extensions/event_bus.md), [guest event channels](extensions/guest.md#event-channels) |
| Model invocation, streaming and provider registration | [Guest model streams](extensions/guest_streams.md) |
| Dialogs, widgets, editors, overlays and custom rendering | [Guest UI](extensions/guest_ui.md), [UI and headless results](extensions/ui.md) |
| Child processes, waits, HTTP bodies, watches and built-in tool wrappers | [Guest native operations](extensions/guest_operations.md) |
| Files, environment, network access and platform limitations | [Operating environment](extensions/host_io.md) |
| Discovery, initialization, component faults and deployment | [Loading](extensions/loading.md), [native execution](extensions/native-execution.md), [packages](packages.md) |

### State, errors and modes

Keep ephemeral state in the factory's captures. Each loaded component gets a
fresh instance, so reloading discards that state unless you wrote it to session
entries or another explicit store. Restore session-backed state from the active
branch rather than from a separate cache that can disagree after a fork.

Use `Result` to propagate rejected host operations. A factory error fails that
extension's load; the loader records the path and continues to later extensions.
A callback error follows its event's dispatch policy, not one blanket fallback:
for example, tool-call errors propagate to the caller while many notification
errors go to error listeners. Event edits are not automatically rolled back.
A component trap faults that instance and affects its later calls. The
[hooks](extensions/hooks.md) and [native executor](extensions/native-execution.md)
pages give the exact rules.

Check `has_ui` before relying on interaction. Headless dialogs return their
defined no-choice result and display setters do nothing; they do not acquire a
terminal implicitly. RPC routes supported dialogs through the client protocol,
not an in-process terminal. See [headless UI](extensions/ui.md#headless-results),
[RPC extension dialogs](rpc.md#extension-dialogs) and
[print mode](print.md). An extension that works in interactive mode still needs
to handle absent UI when used by a script or an embedding.

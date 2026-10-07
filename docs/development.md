# Development

Repository rules are in [AGENTS.md](../AGENTS.md). Commands use the pinned Rust
toolchain and the development tools in `mise.toml` and `mise.lock`.

## Setup and build

```sh
git clone https://github.com/Orchestration-Maestro/maestro.git
cd maestro
mise install
mise exec -- just setup
mise exec -- just build
```

`setup` installs the commit, merge-commit and commit-message hooks. `build`
compiles the delivered workspace through Cargo's dependency graph. `check`
formats, runs Clippy, builds strict public documentation and verifies workspace
conventions. `ci` runs `check`, then `test`. `prepublish` runs `clean`, `build`
and `check`; it never publishes. `clean` removes Cargo outputs, not source files.

## Source invocation

```sh
mise exec -- just --justfile /path/to/maestro/justfile run-source --help
mise exec -- just --justfile /path/to/maestro/justfile run-source --no-env --help
mise exec -- just --justfile /path/to/maestro/justfile run-source-windows --NO-ENV --help
```

The caller's working directory and literal arguments reach the application.
The launcher uses the checkout manifest and pinned Cargo, not a global installed
application. `--no-env` removes provider credentials for that child only; it does
not move authentication files. The Windows form compares the switch without ASCII
case sensitivity. The executable is not delivered yet; its implementation is
tracked in [#249](https://github.com/Orchestration-Maestro/maestro/issues/249).
Until then the source commands report Cargo's missing-package failure.

## Compile watches

```sh
mise exec -- just dev
mise exec -- just models-dev-compile
```

The pinned external watcher compiles initially and queues a rebuild when input
changes during compilation. Earlier terminal output remains visible. It watches
source and manifest inputs, not Cargo outputs. `dev` covers delivered owners;
`dev-compile` and `models-dev-compile` select models only. Missing owners fail.
Compiler watches do not run catalog generation or copy assets. Browser selections
are activated by [#113](https://github.com/Orchestration-Maestro/maestro/issues/113).
The catalog owner's package build will invoke its generator when
[#135](https://github.com/Orchestration-Maestro/maestro/issues/135) supplies it;
ordinary Cargo compilation, tests and watches remain generation-free.

## Identity and execution modes

The application will support source checkout, installed package and standalone
execution. Configuration determines its name, configuration directory and
executable identity, including banners, environment names and configuration paths.
Those configuration and central asset-path functions are owned by
[#216](https://github.com/Orchestration-Maestro/maestro/issues/216); no substitute
configuration schema or asset lookup is provided by the development commands.
Use the owner's central asset paths, not the location of the current source file.
Installed and standalone application execution is not delivered yet.

`copy-assets SOURCE DESTINATION` copies prepared library themes, images, export
templates and vendor assets. `copy-binary-assets SOURCE DESTINATION VIEWER` resolves
the `maestro` manifest through Cargo metadata and copies metadata, README,
release-generated history, themes, images, the supplied viewer, docs and examples.
These commands do not generate a viewer or regenerate assets. Missing inputs fail;
already copied groups and unrelated destination files remain. `app-build` compiles
before library copying. `build-binary` compiles toolkit, models, agent, application
and executable in that order before standalone copying. The browser's bundle
build is owned by #113; the export consumer is owned by
[#192](https://github.com/Orchestration-Maestro/maestro/issues/192).

## Debugging

Interactive debug output, when delivered, belongs under
`~/.maestro/agent/maestro-debug.log`. It contains rendered terminal lines with ANSI
sequences and the last model messages. The interactive owner supplies this
behavior; development commands do not create a replacement logger.

## Tests

```sh
mise exec -- just test
mise exec -- just test-offline
mise exec -- just test -p maestro-tooling maestro_source_without_switch_preserves_environment
```

Ordinary tests use Cargo/libtest directly, including filters, ignored cases and
doctests. Only `test-offline` sets local-model suppression, removes its explicit
provider variables and temporarily moves `$HOME/.maestro/agent/auth.json` to the
fixed backup path, restoring a regular backup on every ordinary return. It runs
`just test` in the invocation directory and ignores wrapper arguments. It is not
a universal sandbox. Do not use it alongside another process editing that auth
file. Tooling regressions instead use disposable authentication fixtures and
controlled processes, without provider calls or operator credentials.

## Package commands and roles

`models-`, `agent-`, `tui-` and `app-` each offer `clean`, `build`, `dev`, `test` and
`prepublish` forms. Package prepublish is clean then build, without root checks.
`tui-test-ansi` explicitly selects terminal wrapping tests. These commands fail
when their owning crate is absent; no placeholder product package is installed.

- `maestro-models`: model interfaces, providers and catalog data.
- `maestro-agent`: conversation execution.
- `maestro-tui`: terminal components.
- `maestro-app`: shared application operations.

Currently `maestro-tooling` automates repository development and
`maestro-test-conventions` verifies workspace structure. Neither ships with the
application. Their integration tests import their libraries.

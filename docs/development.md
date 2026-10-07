# Development

See [AGENTS.md](../AGENTS.md) for additional guidelines.

## Setup

```bash
git clone https://github.com/Orchestration-Maestro/maestro
cd maestro
just setup
just build
```

Run from source:

```bash
/path/to/maestro/scripts/run-source.sh
```

The script can be run from any directory. Maestro keeps the caller's current working directory. PowerShell uses `scripts/run-source.ps1`.

## Forking / Rebranding

Configure via `distribution/npm/maestro-app/package.json`:

```json
{
  "maestroConfig": {
    "name": "maestro",
    "configDir": ".maestro",
    "bin": "maestro"
  }
}
```

Change `name`, `configDir`, and `bin` for your fork. These affect the CLI banner, configuration paths, and environment variable names. The metadata is author scaffolding; the application configuration owner activates these product effects.

## Path Resolution

Three execution modes: npm install, standalone binary, Rust source-build launcher.

**Always use the application's `src/config.rs`** for package assets:

```rust
use crate::config::{get_package_dir, get_theme_dir};
```

Never use direct source-directory-relative paths for package assets. The application configuration owner delivers these asset helpers; this example is not an implemented API.

## Debug Command

`/debug` (hidden) writes to `~/.maestro/agent/maestro-debug.log`:
- Rendered TUI lines with ANSI codes
- Last messages sent to the LLM

The application diagnostic command owner activates this command; it is not available yet.

## Testing

```bash
just test                                      # Run non-LLM tests without API keys
just test                                      # Run all workspace tests
just test -p maestro-models --test stream_observations  # Run a selected integration test
```

All supported default test routes are isolated; tests use controlled fixtures rather than live providers. See [Rust build tooling](rust_build.md) for runner and platform limits.

## Project Structure

```text
crates/
  maestro-models/ # LLM provider abstraction
  maestro-agent/  # Agent loop and message types
  maestro-tui/    # Terminal UI components (terminal owner activation)
  maestro-app/    # CLI and interactive application (application owner activation)
```

## Package watch selection

`just dev` watches the delivered models and agent packages. `just dev-tsc`
watches models only; browser compiler, stylesheet and example watching activate
with the browser crate. `just dev-package <package>` scopes watching to one
package while Cargo builds its dependencies. The source launcher's `--no-env`
environment applies to Cargo metadata and builds as well as the application.

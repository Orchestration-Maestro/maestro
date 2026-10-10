# Configured package sources

`maestro-packages` manages configured sources and acquires or removes their
contents; see [package installation and removal](packages.md). Supply a live
`SettingsManager` and `PackageOperations`; native callers can use
`NativePackageOperations` with their own shell-selection function and stdout
takeover query.

The direct `parse_git_url` parser accepts explicit HTTP, HTTPS, SSH and Git URLs;
shorthand requires the `git:` prefix. Its result separates the clone address,
host/path identity and optional ref. The manager checks the literal `npm:` prefix
first, then the resource owner's local-path classification, then Git parsing.
Git identity ignores transport and ref; npm identity uses the package name.
Hosted shortcuts select their identity from the URL path, excluding the query;
a fragment can select the ref.

Ordinary local paths use the supplied working directory as their input base;
stored locals use their scope's base. Local storage uses `maestro-path` relative-path calculation from the
user agent directory or project `.maestro` directory; see the
[shared path module](../crates/maestro-path/src/lib.rs) documentation for root handling. The manager trims local
inputs and expands `~`, `~/name` and `~name` using the supplied home operation.
Storage resolves incomplete bases and home-derived paths through the supplied
ambient-context operations. Lexical path operations belong to `maestro-path`;
persistence belongs to `maestro-settings`.

Listing returns user rows before project rows, preserving order, duplicates and
object-entry flags. Existing-content lookup returns a path only when the supplied
existence operation succeeds. Consumed malformed source fields fail; ignored
filter fields remain untouched. A duplicate add or absent remove does not call a
settings setter.

User npm lookup runs the configured command with its leading arguments and
reuses only a nonempty root from the same complete command vector. Only exact
`bun` uses global-bin lookup. An empty first command fails with
`Invalid npmCommand: first array entry must be a non-empty command`. Capture
failures use `Failed to run {command} {arguments}: {error}` and propagate to the
caller rather than reporting missing contents.

```rust
use std::{cell::RefCell, rc::Rc};
use maestro_packages::{DefaultPackageManager, NativePackageOperations,
    PackageManager, PackageManagerOptions};
use maestro_settings::{Settings, SettingsManager};

let settings = Rc::new(RefCell::new(SettingsManager::in_memory(Settings::default())));
let packages = DefaultPackageManager::new(
    PackageManagerOptions {
        cwd: "/project".into(),
        agent_dir: "/agent".into(),
        settings_manager: settings,
    },
    NativePackageOperations::new(|_| false, Rc::new(|| false)),
);
assert!(packages.add_source_to_settings("https://github.com/user/repository", None)?);
assert!(!packages.add_source_to_settings("ssh://git@github.com/user/repository", None)?);
assert!(packages.remove_source_from_settings("https://github.com/user/repository", None)?);
# Ok::<(), std::io::Error>(())
```

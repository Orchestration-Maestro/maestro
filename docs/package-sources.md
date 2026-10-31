# Configured package sources

`maestro-packages` manages configured sources and acquires or removes their
contents; see [package installation and removal](packages.md). Supply a live
`SettingsManager` and `PackageOperations`; native callers can use
`NativePackageOperations` with their own stdout takeover query and a caller-driven
`Rc<tokio::task::LocalSet>`.
For read-only availability and configured execution, see [package updates](package-updates.md).

The direct `parse_git_url` parser accepts explicit HTTP, HTTPS, SSH and Git URLs;
shorthand requires the `git:` prefix. Its result separates the clone address,
host/path identity and optional ref. The manager checks the literal `npm:` prefix
first, then the resource owner's local-path classification, then Git parsing.
Git identity ignores transport and ref; npm identity uses the package name.
Hosted shortcuts select their identity from the URL path, excluding the query;
a fragment can select the ref. A host or path segment equal to `.` or `..`,
including one that percent-decodes to it, is rejected, so the source resolves as
a local path and no checkout directory lies outside its git directory; empty
segments, such as a trailing slash, stay accepted. Components follow the checkout
platform: both slash and backslash separate on Windows; on POSIX, a backslash
is an ordinary name character.

Ordinary local paths use the configured working directory as their input base;
stored locals use their scope's base. Local storage uses `maestro-path` relative-path calculation from the
user agent directory or project `.maestro` directory; see the
[shared path module](../crates/maestro-path/src/lib.rs) documentation for root handling. The manager trims local
inputs and expands `~`, `~/name` and `~name` with the home directory: a nonempty
`HOME`, else the platform's home lookup. Expansion keeps a relative home as written.
Stored sources resolve a relative base or home against the process working directory,
which the manager reads only when a path needs it.
Lexical path operations belong to `maestro-path`;
persistence belongs to `maestro-settings`.

Listing returns user rows before project rows, preserving order, duplicates and
object-entry flags. Existing-content lookup returns a path only when it exists on
the file system. Consumed malformed source fields fail; ignored
filter fields remain untouched. Adding a changed npm pin or Git ref replaces only
the first identity match, in place, preserving other object members and their
order. Same parsed pins and local aliases, or an absent removal, do not call a
settings setter. Install-and-persist also keeps an existing Git spelling when
acquisition found no commit change; an absent registration is still appended.

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
let local = Rc::new(tokio::task::LocalSet::new());
let packages = DefaultPackageManager::new(
    PackageManagerOptions {
        cwd: "/project".into(),
        agent_dir: "/agent".into(),
        settings_manager: settings,
    },
    NativePackageOperations::new(Rc::new(|| false), &local),
);
assert!(packages.add_source_to_settings("https://github.com/user/repository", None)?);
assert!(!packages.add_source_to_settings("ssh://git@github.com/user/repository", None)?);
assert!(packages.remove_source_from_settings("https://github.com/user/repository", None)?);
# Ok::<(), std::io::Error>(())
```

## Git preparation and recovery

Missing checkouts are cloned, checked out and prepared in an exclusively owned
sibling directory, then renamed to the installed path. Preparation or publication
failure removes only that owned stage when cleanup succeeds; cleanup failure is
reported. Dropping the staging guard attempts cleanup, not child cancellation.

Installing again over an existing checkout always contacts the remote. It
fetches only the requested ref, or the upstream branch when no ref is requested,
as a configured update does. When the fetched commit differs from HEAD, the
checkout is reset to it, cleaned and prepared in place using its effective Git
configuration. The branch the checkout has selected is the one that moves; no
other branch is created or selected. A branch or tag that moved upstream
therefore resolves to its new commit.

Before mutation, the manager records HEAD and its attached/detached selection.
Probe failures propagate instead of being mistaken for a detached HEAD. A fetched
commit equal to HEAD leaves files, selection and settings untouched. Failed reset,
clean or dependency preparation restores the original selection and resets to its
original commit, so a retry still prepares dependencies. Failed recovery is
reported and cannot promise restoration. Fetch metadata is not transactional.

A requested ref must exist on the remote. These forms no longer reinstall:

- a branch or tag that exists only in the local checkout;
- a remote-tracking spelling such as `origin/main`;
- a revision expression such as `a...b`.

A ref already present locally is also fetched rather than trusted, and a branch
and tag that share a name are resolved by Git's own ref matching, not by a
preference for the branch.

Live files change during dependency preparation; reset/clean discard dirty and
untracked contents; failed dependencies may leave the previous revision's
dependency directory partly changed after rollback. These operations do not
provide crash recovery or concurrent-writer isolation.

Corrected package defects:

- A malformed resource list in a package manifest no longer fails the package; it is ignored and the other lists load (see [package-root manifests](packages.md#package-root-manifests)).
- Failed fresh acquisition no longer leaves partial contents that a retry skips.
- Explicit version changes no longer leave an old checkout or stale settings pin.
- Failed changed updates restore their previous revision so the next attempt retries.

# Footer metadata

`maestro-app` owns the data a footer shows but does not render: the cached Git
branch, extension status texts and a supplied available-provider count.
`FooterDataProvider` holds them; a frontend reads them through the
`ReadonlyFooterDataProvider` trait, which is re-exported at the crate root.
The trait, the live `ExtensionStatuses` view and its iterator are declared in
`maestro_extensions::footer_data_provider`, because extension footer factories
receive them; the application re-exports them at its existing paths.
Available repository watches refresh Git metadata asynchronously. Frontends
subscribe to working-directory changes and changed cached branches.

```rust
use std::rc::Rc;
use maestro_app::ReadonlyFooterDataProvider;
use maestro_app::presentation_data::footer_data_provider::{
    FooterDataProvider, NativeFooterOperations,
};

let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all().build().unwrap();
let local = Rc::new(tokio::task::LocalSet::new());
let operations = Rc::new(NativeFooterOperations::new(Rc::clone(&local)));
let provider = FooterDataProvider::new(".".to_owned(), operations);
provider.set_extension_status("build", Some("passing"));
let _branch = provider.get_git_branch();
let statuses = provider.get_extension_statuses();
assert_eq!(statuses.get("build").as_deref(), Some("passing"));
local.block_on(&runtime, async { provider.dispose(); });
```

## Branch

`FooterOperations` supplies the watch registration, timer scheduling, polling
and Git lookups, so native and controlled adapters are interchangeable;
`NativeFooterOperations` (Unix and Windows) is the native adapter and other
targets pass their own. Stable Rust has no file change time on Windows, so there
the poll reports the modification time as the change time. Paths stay authored
strings; their lexical rules are those of [`maestro-path`](../../crates/maestro-path/src/lib.rs).

Construction walks from the working directory towards the root for the nearest
`.git` entry. A directory needs a `HEAD`; a file is a worktree link when it
starts with `gitdir: `, and any other file or entry kind continues the walk. A
`.git` entry that cannot be examined, a worktree link or its `commondir` that
cannot be followed and a repository without `HEAD` each end the walk with no
branch instead of falling back to an enclosing repository. `HEAD` is checked
before a worktree link's `commondir`, which a plain directory never reads, and the working directory is read only when a worktree path
still needs it after the recorded drive directories are applied. A getter performs
the first synchronous branch lookup if a refresh has not already populated the
cache. The cached result, absence included, is replaced by subsequent refreshes
or invalidated by a working-directory change.

The branch is `None` outside a repository, the text after `ref: refs/heads/`
unchanged (even when empty) and `detached` for any other readable `HEAD`; an unreadable `HEAD` gives no branch. A repository
that stores the placeholder `.invalid` asks `git --no-optional-locks
symbolic-ref --quiet --short HEAD` in the directory that holds `.git`; a
failed or empty answer gives `detached`. Whitespace around file text is trimmed
with Unicode whitespace rules that trim U+FEFF while retaining U+0085,
U+180E and U+200B.

## Live refresh

Until disposed, the provider watches the directory containing `HEAD`. Registration
proceeds in order and stops at the first failure or absent path: when the common
metadata directory's `reftable` directory is present and registered, it is watched
too, and when its `tables.list` file is then present and registered, that file is
watched and polled every 250 ms. Both presence checks read the file system
directly, and a path that cannot be examined counts as absent. Native polling
paths and asynchronous Git query directories are anchored when their operations
are called, before the task set runs; if absolute resolution fails, the original
path is retained.
HEAD-directory events with an unknown,
empty or exact `HEAD` filename request refresh; reference-table events need no
filename filter. Polling requests refresh when modification time, metadata change
time or size differs. The native poll retains the last successful sample through
a missing file and compares reappearance against that sample.

The first event schedules a 500-ms refresh; later events do not move that
deadline. Events during an unresolved query queue one follow-up, scheduled after
completion. Refresh publishes before notifying subscribers, and an initial
refresh that populates an unread cache does not notify. Unchanged results stay
silent. A completion from an old working directory or a disposed provider cannot
publish, and an event or watch error arriving after disposal neither queues a
refresh nor schedules a retry.

Watch registration or asynchronous watch failure releases the registered sources
and schedules another attempt in five seconds. A watch failure leaves an already
scheduled branch refresh intact. Native notification and timer effects use
[Filesystem watching](../filesystem-watch.md); callers must drive the supplied
`LocalSet` for their lifetime.

## Statuses, count and subscriptions

`ExtensionStatuses` keeps insertion order. A replaced text keeps its place and
a removed key that returns goes to the end. Every clone and iterator shares one
collection, so a retained view sees later changes; an iterator skips entries
removed before its turn, reaches entries added before it ends and stays ended
once it returns `None`.

`set_cwd` does nothing for the same spelling. Otherwise it rediscovers the
repository, cancels old watch deadlines, invalidates the cached branch and
rebinds available watches before calling the branch-change callbacks in
registration order, including callbacks added during the walk.
`on_branch_change` registers each `Rc` once; the returned function removes
that registration; holding it retains the watch lifecycle, and dropping it does
not unsubscribe. `dispose` ends watch work and clears subscriptions while leaving
the branch, statuses and count readable. Changing the working directory after
disposal retains synchronous metadata and notification behavior without opening
new watch resources. Closing one provider's polling registration leaves other
providers' registrations intact.

## Working-directory display

The footer shows the working directory with the home directory spelled `~` only
for the home itself and the paths below it, through `maestro_path::shorten_home`.
A sibling whose name starts with the home's name, such as `/srv/u2/x` under the
home `/srv/u`, keeps its full path, as does any path when the home is empty,
relative or the root directory. The session selector, the tree selector, the
context-file list and the tool call rows use the same function.

# Footer metadata

`maestro-app` owns the data a footer shows but does not render: the cached Git
branch, extension status texts and a supplied available-provider count.
`FooterDataProvider` holds them; a frontend reads them through the
`ReadonlyFooterDataProvider` trait, which is re-exported at the crate root.
Automatic refresh from the file system is not part of this delivery: the branch
changes only when `set_cwd` is called.

```rust
use std::rc::Rc;
use maestro_app::ReadonlyFooterDataProvider;
use maestro_app::presentation_data::footer_data_provider::{
    FooterDataProvider, NativeFooterOperations,
};

let provider = FooterDataProvider::new(".".to_owned(), Rc::new(NativeFooterOperations));
provider.set_extension_status("build", Some("passing"));
let _branch = provider.get_git_branch();
let statuses = provider.get_extension_statuses();
assert_eq!(statuses.get("build").as_deref(), Some("passing"));
```

## Branch

`FooterOperations` supplies the file and process effects, so native and
controlled adapters are interchangeable; `NativeFooterOperations` (Unix) is the
native adapter and other targets pass their own. Paths stay authored strings;
their lexical rules are those of [`maestro-path`](../../crates/maestro-path/src/lib.rs).

Construction walks from the working directory towards the root for the nearest
`.git` entry. A directory needs a `HEAD`; a file is a worktree link when it
starts with `gitdir: `, and any other file or entry kind continues the walk. A
`.git` entry that cannot be examined, a worktree link or its `commondir` that
cannot be followed and a repository without `HEAD` each end the walk with no
branch instead of falling back to an enclosing repository. `HEAD` is checked
before `commondir`, and the working directory is read only when a worktree path
still needs it after the recorded drive directories are applied. Reading `HEAD` waits for
the first `get_git_branch` call, and its result, absence included, is cached
until the working directory changes.

The branch is `None` outside a repository, the text after `ref: refs/heads/`
unchanged (even when empty) and `detached` for any other readable `HEAD`; an unreadable `HEAD` gives no branch. A repository
that stores the placeholder `.invalid` asks `git --no-optional-locks
symbolic-ref --quiet --short HEAD` in the directory that holds `.git`; a
failed or empty answer gives `detached`. Whitespace around file text is trimmed
as JavaScript's `String.prototype.trim` does, so U+FEFF is trimmed while U+0085,
U+180E and U+200B are kept.

## Statuses, count and subscriptions

`ExtensionStatuses` keeps insertion order. A replaced text keeps its place and
a removed key that returns goes to the end. Every clone and iterator shares one
collection, so a retained view sees later changes; an iterator skips entries
removed before its turn, reaches entries added before it ends and stays ended
once it returns `None`.

`set_cwd` does nothing for the same spelling. Otherwise it rediscovers the
repository, invalidates the cached branch and then calls the branch-change
callbacks in registration order, including callbacks added during the walk.
`on_branch_change` registers each `Rc` once; the returned function removes
that registration, dropping it does not. `dispose` removes every subscription
and leaves the branch, statuses and count readable.

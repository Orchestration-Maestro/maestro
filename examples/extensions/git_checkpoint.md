# Git checkpoint

Build with `--example git_checkpoint` using the [build command](README.md#building-an-example). Loading it is
opt-in and needs a component-enabled native host, Git on `PATH`, and a Git
repository with an initial commit and a configured author identity. Use a
disposable checkout first: approving a restore runs `git stash apply` against
the host's working directory and can change tracked files or leave conflicts.
Interactive restoration also needs the host's selector UI.

`tool_result` remembers the current session leaf when one exists. `turn_start`
awaits `git stash create` and associates a nonempty object ID with that entry,
replacing its previous checkpoint. Creating the object leaves the working tree
and stash list alone; it does not include untracked files. A clean tree creates
no checkpoint and keeps any previous one.

Before a fork, a matching checkpoint and an available UI open **Restore code
state?** with **Yes, restore code to that point** and **No, keep current code**.
A selection starting with `Yes` awaits `git stash apply` and reports
`Code restored to checkpoint` only if Git exits with code zero without being
terminated. A nonzero exit or terminated command fails the callback with the
exit code, termination flag and Git's standard error instead; inspect any partial
changes or conflicts before continuing. Dismissal, another choice, a missing
checkpoint or a headless host leave the working tree alone.

`agent_end` clears the in-memory checkpoint map, not the remembered entry or
Git's unreachable objects. There is no persistent recovery catalog, automatic
reset, stash push, or built-in checkpoint policy. The controlled runs load the
built component through the production runner in disposable repositories with
scripted UI choices. They need Git but no provider, credentials or paid service.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- git_checkpoint_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- git_checkpoint_
```

# Auto-commit on exit

When the session shuts down, the extension runs `git status --porcelain` in the
host's working directory. A failed status, as outside a repository, or empty
output leaves the directory alone. Otherwise it stages everything with
`git add -A`, including untracked files, and commits with `git commit -m`.

The message is `[maestro] ` followed by the first line of the newest assistant
reply in the session. That line comes from the reply's first text block; a
reply without text, a session without a reply and an empty first line all use
`Session changes`. A line longer than 50 characters keeps its first 50 and
ends with `...`. After a successful commit, a host with a UI shows
`Committed on exit: <message>`. A failed commit shows nothing and leaves the
changes staged.

Build with `--example auto_commit_on_exit` using the
[build command](README.md#building-an-example). Loading it is opt-in and needs
a component-enabled native host, Git on `PATH`, and a repository with a
configured author identity. Every shutdown with changes creates a commit on the
current branch, signed if the repository's settings ask for it, so start in a
disposable checkout. The controlled runs load the built component through the
production runner in disposable repositories. They need Git but no provider,
credentials or paid service.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- auto_commit_on_exit_
```

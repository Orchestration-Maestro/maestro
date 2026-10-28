# Project rules

At each session start the extension scans `.claude/rules/` in the working
directory for Markdown files at any depth. Every agent run then gets a
`## Project Rules` section in its system prompt. The section lists the paths, such
as `- .claude/rules/frontend/components.md`, and tells the agent to load the ones
that apply with the `read` tool. The extension reads no file contents. When the
scan finds rules, a host with a UI shows `Found N rule(s) in .claude/rules/`.

Each start scans again, so rules added or removed between sessions are picked up.
Folders are read one at a time in name order, so the list is the same on every
machine. Only names ending in `.md` count; `.MD` does not. Other files and
symbolic links, to files or to folders, are skipped. A frontmatter `paths` field is
not read: every Markdown file is listed. A missing folder, or one without
Markdown, adds nothing. A rules path that cannot be read, such as a file where the
folder should be, fails the start handler, and the host reports the error.

Build with `--example project_rules` using the
[build command](README.md#building-an-example). Loading it is opt-in and needs a
component-enabled native host with access to the working directory. The listed
paths go to the model on every run, so keep secrets out of rule file names. The
controlled runs load the built component through the production runner over
disposable folders. They need no provider, credentials or paid service.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- project_rules_
```

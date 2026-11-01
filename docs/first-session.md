# Your first Maestro session

Start in a disposable project or a clean Git working tree. Maestro can run shell
commands and change files; a request to leave files alone is not a sandbox.
This guide uses the native source executable on Unix. On Windows, read
[Windows setup and runtime limits](windows.md) before choosing a launch route.
See [command-line input](cli.md) for the CLI reference. If modified Enter keys do
not reach the editor, follow [terminal key forwarding](terminal/setup.md).

## 1. Build from source

Install [rustup](https://rustup.rs/) and [mise](https://mise.jdx.dev/getting-started.html),
plus your platform's native C/C++ build tools. Enable mise in your shell, then run:

```sh
git clone https://github.com/Orchestration-Maestro/maestro.git
cd maestro
mise install
mise exec -- just setup
cargo build --release --locked -p maestro
```

Rustup installs the toolchain pinned in `rust-toolchain.toml`; mise supplies the
development tools. The build uses the checked-in catalog, without fetching a new
model feed. [Development](development.md#setup-and-build) covers the full build
and platform prerequisites.

Keep the checkout: the executable uses its resources. In this shell, define a
shortcut to the source executable before changing directories:

```sh
maestro() { "$MAESTRO_SOURCE/target/release/maestro" "$@"; }
export MAESTRO_SOURCE="$PWD"
maestro --help
```

The commands below use this shell function. It does not install Maestro
system-wide; redefine it when opening another shell. See
[installation paths](application/configuration.md#installation-paths) before
moving the executable or supplying a different resource directory. The existing
[source launcher](development.md#source-invocation) is another way to run from
a checkout while keeping the caller's working directory.

## 2. Connect an account

Launch from the project you want Maestro to work in:

```sh
mkdir -p ../maestro-practice
cd ../maestro-practice
maestro
```

With no configured model, interactive startup still opens so you can sign in.
Choose one credential route:

- **Subscription:** enter `/login`, choose **Use a subscription**, then a provider.
  Follow its browser instructions; paste the redirect URL if the dialog asks.
  Escape cancels. Provider availability and billing differ: read
  [OAuth providers](models/oauth.md) and the
  [subscription billing notice](chat/commands.md#subscription-billing-notice).
- **API key:** enter `/login`, choose **Use an API key**, select a provider and
  paste its key. Alternatively, set that provider's environment variable before
  starting Maestro. For example, `OPENAI_API_KEY` supplies an OpenAI key; see
  [request credentials](credentials.md) and [OpenAI requests](models/responses.md).
  Do not put a real key in a shared terminal transcript or a command-line flag.

Stored sign-ins go to `~/.maestro/agent/auth.json` by default. A nonempty
`MAESTRO_CODING_AGENT_DIR` selects another configuration root. Keep that directory
private and out of version control. `/logout` removes stored credentials, not
environment variables or model-file configuration. See
[signing in](chat/commands.md#signing-in) for save errors and model selection after
login. The separate [account utility](model_login.md) writes a caller-relative
account file; it is not the interactive session's credential store.

## 3. Get a first result

After signing in, use `/model` to choose a model. Type this in the editor and
press Enter:

```text
Explain what files are in this directory. Do not create or change any files.
```

The default tools are `read`, `bash`, `edit` and `write`. `write` creates or
replaces files, `edit` changes matching text, and `bash` executes commands with
your user permissions. Checkpoint work before allowing changes; cancelling a
response does not undo effects already performed. Additional built-ins `grep`,
`find` and `ls` are off by default. [Tool selection](tools.md#selection-and-order)
describes the presets.

For a session with only the built-in reading/search tools and no discovered
extensions, quit with `/quit` and restart:

```sh
maestro --no-extensions --tools read,grep,find,ls
```

This limits exposed tools; it is not filesystem or network isolation. Do not
load untrusted extensions or run Maestro with permissions you do not intend it
to use.

## 4. Add project instructions and input

Create `AGENTS.md` in your project with rules such as:

```markdown
# Working here

- Ask before changing generated files.
- Explain the checks you ran, including failures.
```

Maestro loads context from its agent directory and from the filesystem root down
to the working directory. It chooses one instruction file per directory, with
`AGENTS.md` preferred to `CLAUDE.md`; see [context files](application/resources.md#context-files)
for the full filename precedence. Global instructions normally live at
`~/.maestro/agent/AGENTS.md`. After editing instructions, enter `/reload` while
idle or restart Maestro. Reload also refreshes extensions, skills, prompts,
themes and keybindings. See [reload](chat/commands.md#reload).

To supply a text file, create `notes.md` with a few disposable notes, then start:

```sh
maestro @notes.md "Extract the decisions from these notes."
```

In the editor, type `@` to search for file references. To send an image, put a
real `diagram.png` in the current directory, select a model that accepts images
and run:

```sh
maestro @diagram.png "Describe this diagram."
```

The default paste-image binding
is Ctrl+V; clipboard support depends on the platform. See
[image paste](chat/interactive-mode.md#image-paste) and
[file preparation](cli.md#files). File arguments send their contents to the model;
check them for secrets first.

For a direct shell command in the editor, try:

```text
!printf 'practice output\n'
!!printf 'local-only output\n'
```

The single `!` records output in model context. `!!` marks it excluded from model
context, but still records it in the session. Neither is a private terminal:
do not print secrets. See [shell input](chat/commands.md#shell-input).

## 5. Choose a model and return later

`/model` opens the model picker. From a shell, list configured models before
choosing an exact provider/ID:

```sh
maestro --list-models
```

Pass a listed reference with `--model provider/id`. A configured credential does
not prove the provider will accept a request. Model patterns, thinking suffixes
and defaults are covered by [model selection](selection.md); `/settings` opens
the interactive preferences described in [settings](settings.md).

Sessions save automatically under `~/.maestro/agent/sessions` by default, grouped
by working directory. Quit, then return from the same project:

```sh
maestro --continue
maestro --resume
```

Run these separately: `--continue` opens the most recent session for this
project; `--resume` opens a picker. Use `--session path-or-id` for a particular
saved session, or `--no-session` for an unsaved run. `/resume` is also available
inside chat. See [session storage](sessions/session_manager.md) and
[resume](chat/commands.md#resuming-a-session). Session histories can contain file
contents, prompts and command output, not just the visible final answer.

## Next: one-shot output or integration

After configuring credentials and a model, these run a prompt and exit:

```sh
maestro --print --no-tools "Reply with a short greeting."
maestro --mode json --no-tools "Reply with a short greeting."
```

Text print mode writes the final answer; JSON mode emits session events as JSON
lines, not one JSON document. Redirected stdin also selects print mode. See
[print mode](print.md) for output and exit-status differences.

For a program controlling a long-lived child, start `maestro --mode rpc` and
exchange protocol records over stdin/stdout. It is not an interactive chat
prompt. Use the [RPC guide](rpc.md) and [client library](client.md) rather than
parsing terminal output.

To share a session, review a copy first. Remove credentials, private source,
personal details and sensitive shell output, including excluded `!!` output.
Obtain permission for other people's material. `/share` uploads an HTML export
to a secret GitHub gist; anyone with its URL can read it. It is not a redaction
step. The [offline viewer is still being delivered](https://github.com/Orchestration-Maestro/maestro/issues/1762),
so a checkout without the export bundle cannot complete `/export` or `/share`.
See [sharing behavior](chat/commands.md#share) before publishing. Do not upload
`auth.json` or an unreviewed session directory.

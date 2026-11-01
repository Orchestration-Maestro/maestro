# Daily use

Run `maestro` in a project directory to open chat. Run `maestro --help` to inspect
this build's flags, including flags registered by loaded extensions. For source
builds, use `cargo run --locked -p maestro --` in place of `maestro`; see
[development setup](development.md). A terminal on stdin selects chat; redirected
stdin selects print mode. Native Windows interactive launch is not yet wired:
[Windows runtime owner](https://github.com/Orchestration-Maestro/maestro/issues/486).
Do not infer Android support from the terminal library:
[Android qualification](https://github.com/Orchestration-Maestro/maestro/issues/2818)
is still open.

## Work in the editor

The startup header lists shortcuts and loaded resources. The transcript below it
holds messages, tool calls and results, shell output and notices. Pending messages
appear above the editor. The footer reports the directory, session name, usage,
cost, context and selected model. The editor border indicates thinking level, or
shell mode when the draft starts with `!`. A selector or extension dialog can
replace the editor until it closes.

| Task | Default control or input |
| --- | --- |
| Send a prompt | Enter |
| Add a line without sending | Shift+Enter; terminal key forwarding must preserve it |
| Find a project file | Type `@` and part of its path; select a completion |
| Complete a path | Tab |
| Recall a prompt | Up from an empty editor; Up/Down browse history |
| Edit a large draft | Ctrl+G opens the command in `VISUAL`, falling back to `EDITOR` |
| Attach a clipboard image | Ctrl+V (Alt+V on Windows); inserts a temporary image path |
| Expand tool output and other collapsible rows | Ctrl+O |
| Show or hide thinking | Ctrl+T |
| Select a model | Ctrl+L or `/model` |
| Cycle model / thinking level | Ctrl+P / Shift+Tab |
| Inspect the active shortcuts | `/hotkeys` |

Bindings are configurable in `<agent>/keybindings.json`; `/hotkeys` reflects the
loaded bindings. Terminal shortcuts can intercept modified Enter before Maestro
sees it. See [key decoding](terminal/keys.md), [editor behavior](terminal/editor.md)
and [application keybindings](application/startup.md). The editor supports word movement,
kill/yank, undo and multiline paste. Large pastes collapse to markers but submit
their full text. A custom extension editor may have different controls.

For an external editor, set a command that waits for editing to finish, for example
`EDITOR=vi maestro`. Save and exit with status zero to replace the draft; a failed
launch or nonzero exit keeps it. Unix splits the editor command on spaces, so a
quoted executable path containing spaces is not a shell-quoting interface. The
[external editing owner](chat/interactive-mode.md#external-editing) describes the
temporary file and cleanup. Image clipboard reads may return nothing without a
notice. Dragging a file into a terminal is terminal-specific: check the inserted
path before submitting. Use explicit `@file` arguments for initial attachments.

### Shell commands and tool access

Enter `!pwd` to run a shell command in the session directory and include its output
in later model context. Enter `!!pwd` to record the output but exclude it from
model context. Both still execute locally and remain in the session. Escape
cancels a shell command when no model response is streaming; with both active,
Escape stops the response first. Shell output can contain secrets. Exclusion is
not deletion or redaction.

The default tools are `read`, `bash`, `edit` and `write`. `grep`, `find` and `ls`
are available but off by default. Tools run with your process's access; a prompt
instruction is not a filesystem sandbox or a permission confirmation. Review
extensions and packages before loading them. They can register tools and execute
host operations. Run untrusted work in an isolated OS environment.

```sh
maestro --tools read,grep,find,ls -p "Review the configuration files"
maestro --no-tools -p "Explain the attached note" @note.md
```

The first allowlist removes built-in shell and file-changing tools. It does not
isolate extensions, secret-helper commands or other host effects. `--offline`
limits startup networking, not model calls or tools. See [tool execution](tools.md),
[package trust](packages.md) and [shell input](chat/commands.md#shell-input).

## Redirect a running response

| Intent | Action | Result |
| --- | --- | --- |
| Correct the current run | Type a correction and press Enter | Steering waits for the current assistant turn's tool calls to finish |
| Ask for work after this run | Type a request and press Alt+Enter | Follow-up waits until ordinary continuation and steering finish |
| Edit pending requests | Alt+Up | Restores all queued texts, steering first, then follow-ups, before the current draft |
| Stop generation | Escape | Restores queued text and aborts the response; completed tool effects remain |
| Stop compaction or an automatic retry | Escape while its progress notice is active | Cancels that operation |

For example, while a review runs, send `Focus on file writes` with Enter. Send
`Afterward, list the tests to run` with Alt+Enter. Read the pending rows before
leaving the terminal. Alt+Up removes those pending requests from the queues and
puts them back in the editor for revision.

`steeringMode` and `followUpMode` select `all` or `one-at-a-time` delivery (both
default to `one-at-a-time`). Change them in `/settings`; see
[delivery settings](sessions/settings.md#delivery-and-transport). Input during
compaction waits until it finishes. Registered extension commands run immediately
rather than waiting in the queue. A cancellation cannot undo a file write or an
external command that already ran. [Queue details](chat/interactive-mode.md#queued-input)
cover draft restoration and failures.

## Commands available in chat

Type `/` for completion. Names are case-sensitive. Use the exact forms below:
extra arguments to an argumentless command can become a model prompt rather than
a command. Extensions add commands; `/skill:<name> [arguments]` loads a discovered
skill, and `/<template> [arguments]` expands a prompt template. See
[skills](resources/skills.md) and [templates](resources/prompt-templates.md).

| Command | Use |
| --- | --- |
| `/login`, `/logout` | Choose a provider and configure or remove its stored credentials |
| `/model [term]` | Select a model; a unique exact reference switches directly |
| `/thinking [level]` | Pick or set a level the current model supports |
| `/scoped-models` | Edit models used by cycling; save in the panel to persist the scope |
| `/settings` | Change thinking, theme, queues, transport and display preferences |
| `/new` | Start a new session |
| `/resume` | Browse saved sessions; the selector can include other projects |
| `/name [name]` | Set the session display name, or show its current name |
| `/session` | Inspect session path, ID, message counts, tokens and cost |
| `/tree` | Select a point in the current session tree and continue there |
| `/fork` | Pick an earlier user message; copy the history before it to a new session and edit that message |
| `/clone` | Copy the active branch through its current leaf into a new session |
| `/compact [instructions]` | Summarize older context without deleting raw history |
| `/copy` | Copy the last assistant text to the clipboard |
| `/export [path]` | Export the session as HTML; an exact `.jsonl` suffix exports the active branch as JSONL |
| `/import <path.jsonl>` | Confirm copying a saved session into session storage and replacing the current one |
| `/share` | Export HTML and publish a secret GitHub gist using `gh` |
| `/reload` | Reload keybindings and resources, including context files; refused during streaming or compaction |
| `/hotkeys` | Display configured shortcuts |
| `/changelog` | Display release history |
| `/debug` | Write a terminal/transcript diagnostic log; inspect it for secrets before sharing |
| `/quit` | Shut down, including while a response streams |

The [command reference](chat/commands.md) describes confirmations, cancellation,
errors and extension vetoes. [Settings](sessions/settings.md) lists preference
paths, types and defaults. A dialog's Escape normally closes it without a new
prompt; scope edits already applied to the session remain until changed.

## Keep or branch a session

Sessions save automatically under `<agent>/sessions/`, grouped by working
directory. The default agent directory is `~/.maestro/agent`. Use `/session` to
get the actual file and ID rather than reconstructing an encoded project path.
A new session may not have a saved file until it has content.

```sh
maestro -c                       # Continue the newest session for this project
maestro -r                       # Choose a saved session
maestro --session <path-or-id>    # Open a file or matching partial UUID
maestro --fork <path-or-id>       # Copy it into a new session for this project
maestro --no-session             # Keep this run in memory only
```

Replace `<path-or-id>` with a value from `/session`. Do not combine `--fork` with
`--session`, `--no-session`, continue or resume. Opening another project's session can require
confirmation; an explicit CLI fork copies it into the current project.
`--no-session` prevents session persistence, not tool writes, temporary files or
configured credential effects.

To try branching, review a file, name the session with `/name configuration review`,
and open `/tree`. Selecting a previous point continues in
the same file; choose whether to summarize the branch you leave. `/fork` instead
puts the selected earlier user message into a new session's editor so you can
rewrite it. `/clone` preserves the active branch through the current leaf in a
new file. `/resume` returns to a saved session. Compaction reduces model context;
it is not a history eraser. See [sessions](sessions/sessions.md),
[tree controls](chat/tree-selector.md) and [compaction](compaction.md).

## Choose instructions and attachments

The following table uses `<agent>` for the selected agent directory and `<cwd>`
for the working directory. Settings in `<cwd>/.maestro/settings.json` override
user preferences one level deep; the [settings guide](sessions/settings.md)
explains what each preference controls.

| File or directory | Purpose |
| --- | --- |
| `<agent>/settings.json`, `<cwd>/.maestro/settings.json` | Global and project preferences |
| `<agent>/auth.json` | Stored provider credentials; do not commit it |
| `<agent>/models.json` | Custom provider/model definitions and overrides |
| `<agent>/keybindings.json` | User shortcut overrides |
| `<agent>/AGENTS.md` (or accepted alternate name) | Global project instructions |
| `AGENTS.md` along the root-to-`<cwd>` directory chain | Project instructions, in that order |
| `<cwd>/.maestro/SYSTEM.md`, else `<agent>/SYSTEM.md` | Replace the default system prompt |
| `<cwd>/.maestro/APPEND_SYSTEM.md`, else `<agent>/APPEND_SYSTEM.md` | Append instructions without replacing the default prompt |
| `<agent>/{skills,prompts,themes,extensions}/`, `<cwd>/.maestro/{skills,prompts,themes,extensions}/` | User/project resources; see the linked discovery owner for additional routes |
| `<agent>/sessions/` | Default session history, grouped by project |
| `<agent>/bin/` | Managed search programs used by tools and file completion |

In each instruction directory the loader tries `AGENTS.md`, `AGENTS.MD`,
`CLAUDE.md`, then `CLAUDE.MD`, keeping the first readable existing file. Global
instructions precede ancestor/project instructions. `--no-context-files` (`-nc`)
skips this discovery. `--system-prompt` overrides the `SYSTEM.md` choice;
`--append-system-prompt` overrides the discovered append file and can repeat.
Both accept text or an existing file path. Replacing the system prompt still
appends context files, applicable skill descriptions, date and directory.
[Resource discovery](application/resources.md) owns the complete ordering.

Use context files for stable project conventions. Use a prompt for the current
request. Inspect loaded resources in the header; `/reload` refreshes them after
an edit when the session is idle.

```sh
maestro @note.md "Identify the unanswered question"
maestro @src/config.rs @tests/config.rs "Compare the defaults"
maestro -p @diagram.png "Describe the connections"
printf 'Review this text.\n' | maestro -p "List ambiguous terms"
maestro --system-prompt ./instructions.md --append-system-prompt "Use short answers"
```

Supply your own existing files. Quote a file token containing spaces as
`'@design notes.md'`. Text attachments become file wrappers in the first message;
supported images become image blocks and may be resized. Image failure can leave
an omission note. Select a model that supports image input. Piped input, file
text and the first prompt combine into the initial message; later positional
messages run in order. Missing files fail rather than silently dropping them.
The interactive `@` completion inserts a path reference; initial CLI `@file`
processing explicitly attaches content. [CLI preparation](cli.md#files) and
[model image input](models/api.md#images-and-cross-provider-histories) describe the limits.

## Export without publishing by accident

Use `/export review.jsonl` for a portable copy of the current branch. Back up
files before exporting to an existing destination: export overwrites it. Use
`/import review.jsonl` to copy it back after confirmation. Keep private session
files private; they can contain prompts, code, tool output and credential-bearing
URLs.

HTML export (`/export review.html`, or `maestro --export session.jsonl review.html`)
requires prepared viewer assets. An ordinary Cargo build does not supply them.
The serializer needs `template.html`, `template.css`, `template.js`,
`vendor/marked.min.js` and `vendor/highlight.min.js` in the selected export asset
directory. Do not replace missing assets with downloaded scripts from an
unreviewed source. See [export](export.md) for the supplied-bundle contract,
[offline viewer delivery](https://github.com/Orchestration-Maestro/maestro/issues/1762)
and [prepared asset packaging](https://github.com/Orchestration-Maestro/maestro/issues/2847).

`/share` checks `gh auth status`, exports HTML, then runs `gh gist create
--public=false`. It requires the same assets and a logged-in GitHub CLI. A secret
gist is accessible to anyone with its URL; it is not confidential storage. Review
the export before publishing. The compiled hosted-service origin is empty, so
successful sharing reports a gist URL but no hosted viewer link. It also disables
hosted latest-version and install-report requests. There is no user CLI flag for
that origin; [hosting and domain selection](https://github.com/Orchestration-Maestro/maestro/issues/2846)
need an owner decision. Do not expect an exported viewer or a hosted URL until
those owners deliver them. Local JSONL export does not require the HTML bundle.

## CLI reference

```text
maestro [options] [@files...] [messages...]
```

Use separate option/value tokens, not `--name=value`. Quote shell globs and
comma-separated patterns. Unknown long flags are offered to extensions and can
produce diagnostics; do not rely on misspelled flags being ignored. Repeated
scalar options keep the last value; resource paths and appended prompts accumulate.

### Modes and general options

| Option | Meaning and default |
| --- | --- |
| No mode flag | Chat on terminal stdin; print with redirected stdin |
| `--mode text\|json\|rpc` | Text is the default output format; JSON emits session events, RPC serves stdin/stdout |
| `--print`, `-p` | Process prompts and exit instead of chat; can accompany JSON mode |
| `--export <input> [output]` | Export a saved session to HTML and exit; requires prepared assets |
| `--help`, `-h` | Show help and loaded extension flags |
| `--version`, `-v` | Show the build version |
| `--verbose` | Expand startup output even with `quietStartup` |
| `--offline` | Disable startup network operations, including missing search-helper downloads; does not block model/tool network access |
| `--system-prompt <text-or-file>` | Replace the default prompt; otherwise discover `SYSTEM.md` |
| `--append-system-prompt <text-or-file>` | Repeatable appended text; otherwise discover `APPEND_SYSTEM.md` |

Text print writes the final assistant text. JSON writes JSONL session records;
RPC accepts commands rather than plain prompt text. Help and other ordinary
messages can go to stderr when stdout is reserved. See [print](print.md) and
[RPC](rpc.md) before treating stdout as a protocol stream.

### Model and session options

| Option | Meaning and default |
| --- | --- |
| `--provider <id>` | Qualify model selection; otherwise use configured/available selection |
| `--model <pattern>` | ID, fuzzy pattern, `provider/id`, or a pattern with `:<thinking>` |
| `--api-key <key>` | Override request key; requires an explicitly selected CLI or scoped model; avoid secrets in shell history/process arguments |
| `--thinking <level>` | `off`, `minimal`, `low`, `medium`, `high`, `xhigh`; overrides model-pattern effort; model support still applies |
| `--models <patterns>` | Comma-separated cycling scope; otherwise saved `enabledModels` or available models |
| `--list-models [search]` | List available descriptors, optionally fuzzy-filtered; does not invoke a model |
| `--continue`, `-c` | Continue the newest project session |
| `--resume`, `-r` | Open the session picker |
| `--session <path-or-id>` | Open a saved session, including partial UUID lookup |
| `--fork <path-or-id>` | Copy a saved session into a new session |
| `--session-dir <dir>` | Storage/lookup directory; overrides session environment and setting |
| `--no-session` | In-memory session; by default sessions persist |

Without explicit selection, existing-session choices or saved defaults can apply;
there is no single fixed default model. See [selection](selection.md) for matching,
effort precedence and credential checks. For example, after inspecting
`maestro --list-models`, replace the references below with available models:

```sh
maestro --model openai/gpt-4o --thinking off "Review the test names"
maestro --models 'anthropic/*,openai/*' --thinking low
maestro --model 'sonnet:high' "Check the locking design"
```

### Tools and resources

| Option | Meaning |
| --- | --- |
| `--tools <names>`, `-t <names>` | Comma-separated allowlist across built-in, extension and custom tools |
| `--no-tools`, `-nt` | Disable all tools by default; an explicit allowlist can still enable named tools |
| `--no-builtin-tools`, `-nbt` | Disable built-ins by default while retaining extension/custom tools |
| `--extension <source>`, `-e <source>` | Repeatable explicit extension source |
| `--no-extensions`, `-ne` | Disable automatic extension discovery; explicit sources still load |
| `--skill <path>` | Repeatable skill file/directory |
| `--no-skills`, `-ns` | Disable discovered skills; explicit paths still load |
| `--prompt-template <path>` | Repeatable prompt file/directory |
| `--no-prompt-templates`, `-np` | Disable discovered prompts; explicit paths still load |
| `--theme <path>` | Repeatable theme file/directory |
| `--no-themes` | Disable discovered themes; explicit paths still load |
| `--no-context-files`, `-nc` | Disable automatic context-file loading |

To load only an explicit resource, use `maestro --no-extensions -e ./review.wasm`
with a trusted, built component, or `maestro --no-skills --skill ./review/SKILL.md`
with an existing skill. Explicit resources are not blocked by their discovery
switches. See [resources](application/resources.md) and
[extension loading](extensions.md).

### Package commands

Package commands run before ordinary prompt startup. They can modify installed
resources and settings, so inspect the source and intended scope first.

| Command | Effect |
| --- | --- |
| `maestro install <source> [-l]` | Install/register a source; `-l` selects project settings instead of global |
| `maestro remove <source> [-l]` | Remove a source from the selected scope |
| `maestro uninstall <source> [-l]` | Alias for remove |
| `maestro update [source\|self\|maestro]` | Update one source, the application, or by default both application and packages |
| `maestro update --extensions` | Update packages only; pinned sources are skipped |
| `maestro update --self` | Update the application only |
| `maestro update --extension <source>` | Update one source |
| `maestro update --force` | Force application reinstall when selected |
| `maestro list` | Show configured sources |
| `maestro config` | Enable/disable package resources in a selector |

`--local` is the long form of `-l` for install/remove/uninstall. Package help also
accepts `-h`. Use `maestro install --help` (or another package command's `--help`)
for its grammar. No automatic self-update route is promised for an unmanaged source
build; see [package management](packages.md) and
[self-update selection](application/configuration.md#self-update-selection).

### Environment

These tables cover CLI controls and provider-key discovery. Provider-specific
endpoint, cloud credential and proxy controls are grouped below with their
runtime references. Unset values use the defaults shown; credentials remain
subject to [stored-key precedence](credentials.md) and
[request authentication](request-authentication.md).

| Variable | Effect |
| --- | --- |
| `MAESTRO_CODING_AGENT_DIR` | Nonempty override for `<agent>`; default `~/.maestro/agent`; leading `~/` expands |
| `MAESTRO_CODING_AGENT_SESSION_DIR` | Session root; below `--session-dir`, above the stored session-directory setting |
| `MAESTRO_PACKAGE_DIR` | Override installed resource/package directory; not an extension search-path list |
| `MAESTRO_OFFLINE` | `1`, `true`, `yes` select startup offline mode (case-insensitive words) |
| `MAESTRO_SKIP_VERSION_CHECK` | Nonempty value skips the hosted version check |
| `MAESTRO_TELEMETRY` | Override install reporting; `1`/`true`/`yes` enable, other supplied values disable; does not disable update checks |
| `MAESTRO_CACHE_RETENTION` | Exact `long` requests extended prompt caching where supported; otherwise the ordinary short policy applies |
| `VISUAL`, `EDITOR` | External editor command; first nonempty value wins in this order |
| `MAESTRO_TIMING` | Exact `1` prints startup timing diagnostics |
| `MAESTRO_STARTUP_BENCHMARK` | `1`/`true`/`yes` initialize and stop chat; other modes reject the benchmark request |
| `MAESTRO_OAUTH_CALLBACK_HOST` | Override the native OAuth callback bind host; see [OAuth](models/oauth.md) |

The lower-level hosted-service operations treat any nonempty offline variable as
offline when no caller selection is supplied. CLI startup selects its boolean
once. Both `maestro --offline` and `MAESTRO_OFFLINE=1 maestro` prevent downloads
of missing `fd` and `rg` helpers during interactive startup. Installed helpers
and commands already on `PATH` remain available.
Offline mode is not a network sandbox. The empty compiled service origin prevents
hosted requests independently of these preferences.

| Provider | Key variables, in precedence order |
| --- | --- |
| Anthropic | `ANTHROPIC_OAUTH_TOKEN`, `ANTHROPIC_API_KEY` |
| OpenAI / Azure OpenAI Responses | `OPENAI_API_KEY` / `AZURE_OPENAI_API_KEY` |
| GitHub Copilot | `COPILOT_GITHUB_TOKEN` |
| Google / Google Vertex | `GEMINI_API_KEY` / `GOOGLE_CLOUD_API_KEY` |
| DeepSeek / Groq / Cerebras | `DEEPSEEK_API_KEY` / `GROQ_API_KEY` / `CEREBRAS_API_KEY` |
| xAI / Fireworks / OpenRouter | `XAI_API_KEY` / `FIREWORKS_API_KEY` / `OPENROUTER_API_KEY` |
| Vercel AI Gateway / ZAI / Mistral | `AI_GATEWAY_API_KEY` / `ZAI_API_KEY` / `MISTRAL_API_KEY` |
| MiniMax / MiniMax China | `MINIMAX_API_KEY` / `MINIMAX_CN_API_KEY` |
| Moonshot (both regions) / Hugging Face | `MOONSHOT_API_KEY` / `HF_TOKEN` |
| OpenCode / OpenCode Go | `OPENCODE_API_KEY` (shared) |
| Kimi Coding / Cloudflare | `KIMI_API_KEY` / `CLOUDFLARE_API_KEY` |
| Xiaomi / Token Plan China | `XIAOMI_API_KEY` / `XIAOMI_TOKEN_PLAN_CN_API_KEY` |
| Xiaomi Token Plan Amsterdam / Singapore | `XIAOMI_TOKEN_PLAN_AMS_API_KEY` / `XIAOMI_TOKEN_PLAN_SGP_API_KEY` |

| Additional environment group | Controls and runtime reference |
| --- | --- |
| Azure endpoint | `AZURE_OPENAI_BASE_URL` or `AZURE_OPENAI_RESOURCE_NAME`; `AZURE_OPENAI_API_VERSION` defaults to `v1`; `AZURE_OPENAI_DEPLOYMENT_NAME_MAP` maps model IDs to deployments; [Responses](models/responses.md) |
| Cloudflare routing | `CLOUDFLARE_ACCOUNT_ID`, plus `CLOUDFLARE_GATEWAY_ID` for AI Gateway; [chat completions](models/chat-completions.md) |
| Bedrock credentials | `AWS_PROFILE`, paired `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, `AWS_BEARER_TOKEN_BEDROCK`, `AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`, `AWS_CONTAINER_CREDENTIALS_FULL_URI`, `AWS_WEB_IDENTITY_TOKEN_FILE`; [cloud conversations](models/cloud-conversations.md) |
| Bedrock policy | `AWS_REGION`, fallback `AWS_DEFAULT_REGION`; `AWS_BEDROCK_SKIP_AUTH`, `AWS_BEDROCK_FORCE_CACHE`, `AWS_BEDROCK_FORCE_HTTP1`; [cloud conversations](models/cloud-conversations.md) |
| Google cloud scope | `GOOGLE_APPLICATION_CREDENTIALS`, `GOOGLE_CLOUD_PROJECT` or `GCLOUD_PROJECT`, `GOOGLE_CLOUD_LOCATION`, `GOOGLE_CLOUD_QUOTA_PROJECT`; [content generation](models/content-generation.md) |
| Google runtime | `GOOGLE_API_KEY`, `GOOGLE_GENAI_USE_ENTERPRISE`, `GOOGLE_GENAI_USE_VERTEXAI`, `GOOGLE_GEMINI_BASE_URL`, `GOOGLE_VERTEX_BASE_URL`, `GOOGLE_EXTERNAL_ACCOUNT_ALLOW_EXECUTABLES`, `GOOGLE_API_CERTIFICATE_CONFIG`, `CLOUDSDK_CONFIG`; [content generation](models/content-generation.md) |
| Native networking | `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`, `npm_config_http_proxy`, `npm_config_https_proxy`, `npm_config_proxy`, `npm_config_no_proxy` and case variants; selection varies by transport; [Bedrock proxy routing](models/cloud-conversations.md#proxy-routing), [HTTP connectors](libraries/provider-qualification.md) |

Environment credentials are not validated by listing models. Empty key variables
are absent; whitespace values can still count as configured. Cloud ambient
markers prove configuration presence, not a successful request. Do not paste
keys into a diagnostic log or committed example.

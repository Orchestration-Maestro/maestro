# Embedding a session

A program embeds Maestro through `maestro_app`. The crate root re-exports the
session, its creation and replacement operations, and the records of the
compaction, history, resource, settings, tool and theme owners they use. Frontend
modes and the interactive components stay with their own crates; the RPC client,
which drives Maestro as a child process, lives here in `rpc_client`
([RPC client](../client.md)).

[Session observations](lifecycle.md) describes name publication and finalized
message replacements.

## Creating a session

`create_agent_session` takes a `CreateAgentSessionOptions` and fills each absent
input with its default:

- The working directory is the explicit `cwd`, else the supplied session
  manager's directory, else the process directory.
- The agent directory defaults to the user configuration directory. Credentials
  default to `auth.json` in it and the model catalog to `models.json`.
- Preferences default to the settings of the working and agent directories, and
  the history to a new session file for the working directory.
- `resources` is required. `SessionResources::Loader` uses a loader the caller
  built and loaded. `SessionResources::Default` builds the default loader for
  the working and agent directories from the given `ResourceLoaderOptions` and
  loads it first. A caller that supplied no loader passes its task set and
  extension module loader there.
- `offline` is the session's own offline selection for search programs. It is
  separate from `ResourceLoaderOptions::offline`, which stays with configured
  packages; neither overwrites the other. Sessions created from retained
  [`AgentSessionServices`] inherit the selection those services were created with.
- A catalog created here runs the commands that configured credentials name as
  child processes; a supplied catalog keeps its own handle. `theme` is the
  published theme extensions see while no user interface is bound; the default
  slot is unpublished.

The model is the explicit one. Otherwise, when the history holds messages and
records a model, that model is restored if the catalog has it with credentials.
Failing that, the session picks the saved default model, then the first
available one. A failed restoration reports
`Could not restore model <provider>/<id>`, followed by `. Using <provider>/<id>`
when another model was chosen. With no model at all, the message tells the
user how to add one, and the agent keeps its `unknown` placeholder model.

The thinking level is the explicit one, else the recorded level of a resumed
history that holds a thinking entry, else the saved default, else `medium`. It is
clamped to the model, and is `off` without one.

A new history records the model change, when there is a model, and then the
thinking change. A resumed history gives the agent its stored messages, and
records the thinking change only when the branch has none.

`tools` is an allowlist that also lists the initially active tools. Without it,
`NoTools::All` starts with no tool and an empty allowlist, `NoTools::Builtin`
starts without the default built-in tools but keeps extension and SDK tools, and
otherwise `read`, `bash`, `edit` and `write` start active. `custom_tools` are
registered under `<sdk:name>`.

`create_agent_session_from_services` creates a session over services that
already exist: their directories, credentials, catalog, preferences and loader.

## Requests

Each provider request takes the catalog's key and headers for the model; a
credential failure fails the request with its message. The provider retry
settings supply the timeout, the retry limit and the longest retry delay where
the request sets none. Headers merge in order: attribution headers, then the
credential headers, then the request's own, later names replacing earlier
values.

While install telemetry is on, requests to `openrouter` or an `openrouter.ai`
address carry `HTTP-Referer: https://github.com/Orchestration-Maestro/maestro`,
`X-OpenRouter-Title: maestro` and `X-OpenRouter-Categories: cli-agent`. Requests
to Cloudflare's providers or addresses carry `User-Agent: maestro-coding-agent`.

Extensions can replace each request payload through `before_provider_request`,
hear each response status and headers through `after_provider_response`, and
rewrite the request messages through `context`.

`convert_to_llm_with_block_images` converts messages for the model. When the
`images.blockImages` setting is on, each image of a user or tool-result message
becomes the text `Image reading is disabled.`, and consecutive placeholders
collapse into one. The setting is read on every conversion.

## Extension bindings and reload

The session builds its extension runner when it is created and installs its tool
hooks once. The hooks read the runner in effect when they run, so a rebuilt
runtime needs no new hooks. Extensions receive `session_start` only when
`bind_extensions` runs. Each field of `ExtensionBindings` that is present
replaces the bound operation; an absent field leaves it as it was. The UI field
is an `ExtensionUIContextBinding`: an SDK host converted with `.into()` stays
owned by the session, while a host that owns the session supplies
`ExtensionUIContextBinding::borrowed`. Binding then
sends the start event and hands the skill, prompt and theme paths extensions
discover to the loader. An inline extension `<name>` labels them
`extension:name`; a file extension labels them `extension:` and its file name
without `.ts` or `.js`, with its directory as their base.

`reload` captures the flag values, sends the `reload` shutdown, reloads the
settings, resets the provider routes and reloads the resources. It then rebuilds
the runtime with the captured flags and the active tools. Only when a binding is
present does it send a fresh `session_start` with `reload` and discover
resources again. Contexts captured before a reload stay usable. `dispose`
invalidates every extension context, stops agent delivery, drops every listener
and releases the session's provider resources. It aborts nothing.

## Replacing the session

`create_agent_session_runtime` checks that the stored working directory still
exists, creates the first session through the factory and keeps the factory for
every replacement. `AgentSessionRuntime` operations each follow the same order:
extensions may cancel through `session_before_switch` or `session_before_fork`;
the target history is prepared; the current session shuts down with its reason,
the before-invalidate callback runs, and the session is disposed; the factory
creates the next session, the rebind callback runs, and `with_session` continues
with the replacement's context. Setup, message reload and continuation stay bound
to that replacement even if another operation changes the current session while
a callback is suspended. The start event carries the previous session file.
A failure after the teardown restores nothing.

`set_before_session_invalidate` takes a `BeforeSessionInvalidate` callback that
returns a `Result`. When it fails, the operation returns that failure through the
runtime's error type and stops: the session is neither disposed nor replaced, and
the callback's earlier effects stay in place.

- `new_session` starts a history in the same directory, optionally recording a
  parent session, and runs `setup` on it before the agent reloads its messages.
- `switch_session` resumes another file, optionally in another directory, and
  fails with `AgentSessionRuntimeError::MissingSessionCwd` when its stored
  directory is gone.
- `fork` forks before a user message, returning its text, or at any entry.
  `Invalid entry ID for forking` refuses an unknown entry or, before, an entry
  that is not a user message. A stored session forks into a new file; an
  in-memory one branches its own history.
- `import_from_jsonl` copies a file into the session directory unless it is
  already there, then resumes it. A missing file fails with
  `AgentSessionRuntimeError::SessionImportFileNotFound` and `File not found:
  <path>`.
- `dispose` sends the `quit` shutdown, runs the before-invalidate callback and
  disposes the session.

## Account files

`model_account::cli::load_auth` reads the account file at a path the caller
chooses. A missing, unreadable or unparsable file, or one whose root is not an
object, yields no entries. `save_auth` writes the entries as two-space JSON with
no final newline, and integral numbers without a fraction. It refuses with
`AccountFileError::NotAnObject`, leaving the file unchanged, when the stored
root is valid JSON but not an object, and returns write failures as
`AccountFileError::Write`.

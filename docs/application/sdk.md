# Embedding a session

A program embeds Maestro through `maestro_app`. The crate root re-exports the
session, its creation and replacement operations, and the records of the
compaction, history, resource, settings, tool and theme owners they use. Frontend
modes and the interactive components stay with their own crates; the RPC client,
which drives Maestro as a child process, lives here in `rpc_client`
([RPC client](../client.md)).

[Session observations](lifecycle.md) describes name publication and finalized
message replacements. The [executable examples](examples.md) cover model selection,
resources, tools, credentials, persistence and runtime replacement.

## Choose the integration boundary

Use the Rust SDK when your host needs the session's live state, native tools or
extension bindings in the same process. Your host owns the executor, directories,
credentials and shutdown. The session owns conversation processing.

Use [RPC](../rpc.md) when the host is written in another language or needs a
process boundary. The [Rust RPC client](../client.md) also works for a Rust host
that wants Maestro in a child process. Protocol records cross that boundary;
`Rc<AgentSession>` and callbacks do not.

## Start with an isolated run

Work from a Maestro source checkout with its pinned Rust toolchain
([development setup](../development.md)). The native
embedding crate is `maestro-app` (imported as `maestro_app`); this guide does not
assume a published package. In another Cargo workspace, point the `maestro-app` dependency at
`crates/maestro-app` in your checkout. Use the checkout's Tokio version with `rt`, `time` and `sync`
for the executor and completion signal used below. This controlled check also
uses `maestro-models` for its scripted provider and `maestro-test-directory` for
disposable storage; they are checkout crates, not prerequisites for a live host.
The application's manifest already declares these dependencies for its doctests.

The session uses `Local` execution: drive it on a Tokio `LocalSet`, even if the
host has other threads. Keep that task set running until session work has ended.
The loader takes the same task set plus a `ModuleLoader<Local>` callback. Here
that callback returns no extension, discovery is disabled, tools are off, and
both package and search-program policy are offline. The response is scripted;
`controlled-key` is only a non-secret credential for that provider.

Run the checked version of the following program from the checkout root:

```sh
cargo test --locked --doc -p maestro-app
```

It creates separate working and agent directories, uses in-memory credentials,
settings and history, collects streamed text, checks one response, removes its
subscription and disposes the session. It makes no paid or live provider request.
The directory guard removes the disposable files on return.

```rust
use maestro_app::{
    AgentEvent, AgentSessionEvent, AssistantMessageEvent, AuthStorage, ConfigValueOperations,
    CreateAgentSessionOptions, Local, ModelRegistry, ModuleLoader, NoTools, PromptOptions,
    ResourceKind, ResourceLoaderOptions, SessionManager, SessionResources, SettingsManager,
    create_agent_session,
};
use maestro_models::{
    FauxAssistantMessageOptions, FauxResponseStep, RegisterFauxProviderOptions,
    faux_assistant_message, register_faux_provider,
};
use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};

struct NoCommands;
impl ConfigValueOperations for NoCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> { None }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = maestro_test_directory::Scratch::new()?;
    let cwd = root.path().join("project");
    let agent_dir = root.path().join("agent");
    std::fs::create_dir_all(&cwd)?;
    std::fs::create_dir_all(&agent_dir)?;
    let cwd = cwd.to_string_lossy().into_owned();
    let agent_dir = agent_dir.to_string_lossy().into_owned();
    let executor = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let local = Rc::new(tokio::task::LocalSet::new());
    executor.block_on(local.run_until(async {
        let provider = register_faux_provider(RegisterFauxProviderOptions::default());
        provider.set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message("Ready.", FauxAssistantMessageOptions::default()),
        ))]);
        let auth = Arc::new(AuthStorage::in_memory(Default::default()));
        auth.set_runtime_api_key("faux", "controlled-key");
        let registry = Arc::new(ModelRegistry::in_memory(auth.clone(), Arc::new(NoCommands))?);
        let module_loader: ModuleLoader<Local> =
            Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
        let mut resources = ResourceLoaderOptions::new(Rc::clone(&local), module_loader);
        resources.disabled.extend([
            ResourceKind::Extensions, ResourceKind::Skills, ResourceKind::PromptTemplates,
            ResourceKind::Themes, ResourceKind::ContextFiles,
        ]);
        resources.system_prompt = Some("Answer briefly.".into());
        resources.append_system_prompt = Some(Vec::new());
        resources.offline = Some(true);
        let created = create_agent_session(CreateAgentSessionOptions {
            cwd: Some(cwd.clone()), agent_dir: Some(agent_dir),
            auth_storage: Some(auth), model_registry: Some(registry),
            model: Some(Arc::new(RwLock::new(provider.models[0].clone()))),
            thinking_level: None, scoped_models: Vec::new(),
            no_tools: Some(NoTools::All), tools: None, custom_tools: Vec::new(),
            resources: SessionResources::Default(Box::new(resources)),
            session_manager: Some(Rc::new(RefCell::new(
                SessionManager::in_memory(Some(&cwd))?,
            ))),
            settings_manager: Some(Rc::new(RefCell::new(
                SettingsManager::in_memory(Default::default()),
            ))),
            session_start_event: None, theme: Default::default(), offline: Some(true),
        }).await?;
        assert!(created.model_fallback_message.is_none());
        assert!(created.extensions_result.errors.is_empty());
        let session = created.session;
        let text = Rc::new(RefCell::new(String::new()));
        let output = Rc::clone(&text);
        let ended = Rc::new(tokio::sync::Notify::new());
        let completion = Rc::clone(&ended);
        let unsubscribe = session.subscribe(Rc::new(move |event| {
            if let AgentSessionEvent::Agent(event) = event {
                match &*event.read().unwrap() {
                    AgentEvent::MessageUpdate {
                        assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. }, ..
                    } => output.borrow_mut().push_str(delta),
                    AgentEvent::AgentEnd { .. } => completion.notify_one(),
                    _ => {}
                }
            }
        }));
        let result = session.prompt("Report readiness.", PromptOptions::default()).await;
        if result.is_ok() { ended.notified().await; }
        unsubscribe();
        session.abort().await;
        session.dispose();
        provider.unregister();
        result?;
        assert_eq!(&*text.borrow(), "Ready.");
        assert_eq!(provider.state.call_count(), 1);
        Ok::<(), Box<dyn std::error::Error>>(())
    }))?;
    Ok(())
}
```

## Select resources and overrides

Choose the working directory before constructing resources or tools. The
session's `cwd` does not relocate a loader you supplied yourself. An
`agent_dir` is the user configuration root, distinct from the project directory;
[configuration](configuration.md) explains how the default is selected.

For normal discovery, leave `ResourceLoaderOptions::disabled` empty. Supply the
extension module loader your host actually uses; the callback above disables file extension loading. The native component adapter
is described in [extensions](../extensions.md#load-and-reload). Keep all
additional paths absolute if you will reuse options after a directory change.

There are two resource routes:

- `SessionResources::Default` constructs and reloads the default loader before
  returning the session.
- `SessionResources::Loader` accepts your `Rc<dyn ResourceLoader<Local>>` as it
  stands. Build `DefaultResourceLoader` with `DefaultResourceLoaderOptions`,
  call `reload().await`, inspect its results, then pass it. An unloaded loader
  has empty results. A custom implementation controls its own discovery.

[Application resources](resources.md) records reload order, path resolution and
provenance. Use the following loader inputs rather than changing agent state to
customize the assembled prompt:

| Need | Loader input | What to retain |
| --- | --- | --- |
| Replace the base instructions | `system_prompt` or `system_prompt_override` | The assembled prompt still includes context, appended text and eligible skills. |
| Add instructions | `append_system_prompt` or `append_system_prompt_override` | `append_system_prompt` accepts text or file paths; an empty list prevents file discovery. The override returns final text, without resolving paths. |
| Supply skills | `additional_skill_paths` or `skills_override` | Keep diagnostics when extending the existing `LoadSkillsResult`; each skill needs its file path and source. |
| Supply context | `agents_files_override` | Append or replace `ContextFile` records with a path and content. |
| Add slash templates | `additional_prompt_template_paths` or `prompts_override` | Keep template source metadata and diagnostics; the prompt name selects it. |
| Load extensions | `additional_extension_paths` or `extension_factories` | Keep load errors visible, even when other extensions loaded. |

An override receives the loaded value and replaces it with its return value. To
extend a result, mutate that value and return it; returning a new empty result
also discards its diagnostics. See [skills](../resources/skills.md) and
[prompt templates](../resources/prompt-templates.md) for their record formats.
`/skill:<name>` reads the skill body when invoked; `/<template>` expands a
loaded template. These are prompt inputs, not shell commands.

For host-to-extension communication, create a shared bus with
`create_event_bus_for::<Local>` from `maestro-extensions` with a local tail spawner and error reporter, and put its controller
in `ResourceLoaderOptions::event_bus`. Use `for_extension()` for a host handle
and subscribe and emit through it; the
extensions see it as their events interface. This is separate from
`AgentSession::subscribe`, which observes session activity. The caller's task
set drives asynchronous bus continuations. See [event bus](../extensions/event_bus.md).

## Select a model and credentials

Pass `model` for an explicit descriptor, or let the constructor restore or select
one as described below. `get_model` reads a built-in descriptor;
`ModelRegistry::find` also finds custom models. Finding a model does not prove
it has credentials. `get_available` filters the catalog to configured choices.
`thinking_level` selects the requested reasoning level; `scoped_models` limits
cycling choices, with an optional level per choice.

Retain one `Arc<AuthStorage>` and the `Arc<ModelRegistry>` built over it when
credentials must be shared. `AuthStorage::create` uses your chosen file path;
`AuthStorage::in_memory` avoids credential persistence. A runtime API key set
with `set_runtime_api_key` is not saved. SDK request lookup tries a nonempty
runtime key, stored credentials, provider environment values, then the catalog's
configured provider key. SDK requests do not call the credential fallback resolver;
direct `AuthStorage::get_api_key` lookup can include it when `include_fallback`
is `true`. Stored OAuth credentials may refresh during lookup. A malformed stored
record can stop lookup rather than falling through; [credentials](../credentials.md)
describes the exact cases and login callbacks.

`ModelRegistry::create` loads your model file, whereas `in_memory` avoids model
file access. Both take the configured-value command operations. The check above
refuses such commands. A real host that permits them must supply the execution
policy deliberately. Do not embed a live API key in source or log resolved keys.
[The catalog](../catalog.md) describes custom provider registration and headers.

## Limit tools before prompting

`tools: Some(names)` is the allowlist and initial active selection. An empty
list disables every tool, including extension and SDK tools. With no list,
`NoTools::All` disables every tool; `NoTools::Builtin` removes built-in defaults
but leaves extension and SDK tools enabled. Without either selection, the
initial built-ins are `read`, `bash`, `edit` and `write`.

Pass standalone definitions through `custom_tools`; extensions can register
tools too. The session constructs built-ins for its working directory and gives
SDK definitions `<sdk:name>` provenance. Supply a prompt snippet when the tool
should appear in the available-tool description. Custom implementations keep
responsibility for their own path resolution and effects. The
[native tool factories](../tools.md) take an explicit directory; a host-created
tool does not inherit a later session directory change.

After construction, `get_all_tools` exposes definitions and sources, and
`set_active_tools_by_name` changes the active selection and rebuilds the system
prompt. [Session operations](session-operations.md#tools-and-the-system-prompt)
explains registry replacement and extension refresh.

## Control history and settings

Choose history explicitly when persistence is unwanted: `SessionManager::in_memory`
is used above. `create` starts a persistent history, `continue_recent` resumes the
most recent one for the directory, and `open` resumes a selected file. Supply the
result as the session's shared manager. Import `ReadonlySessionManager` for its
tree reads. Use the manager's `get_entries`, `get_tree`, `get_branch` and
`get_entry` to inspect the tree; session navigation
and replacement preserve the conversation semantics that direct leaf edits do
not provide. See [session format](../sessions/session_format.md) and
[session navigation](navigation.md).

`SettingsManager::create` loads global and project preferences;
`SettingsManager::in_memory` supplies an isolated document. The application uses
`<agent_dir>/settings.json` and `<cwd>/.maestro/settings.json`. Project values
override global ones; object merging is one level deep, not recursive.
`apply_overrides` changes effective preferences without saving them. A setter or
reload rebuilds the effective document and discards those overrides. Locked base
settings can refuse overrides; inspect `drain_errors`.

Setters publish memory changes immediately and queue persistence. Retain the
manager if your host needs to await `flush` before exit and report `drain_errors`;
flushing alone does not report failure. Avoid holding a `RefCell` borrow across
that await: obtain the flush future first, release the borrow, then await it.
[Settings](../settings.md) lists defaults, locks and persistence rules.

## Send input and observe completion

`prompt(text, PromptOptions)` starts work immediately; dropping its returned
future does not cancel the run. Await the result and handle its error.
`preflight_result` reports admission once, not success of the model response.
A later provider error is also represented in the response/events; inspect the
assistant's stop reason and error message rather than treating admission as a
successful answer.

Templates expand by default. Options can supply images, the input source and
`streaming_behavior`. During a response, choose steering or follow-up delivery;
an ordinary prompt without a choice is refused. `steer` and `follow_up` provide
explicit queue operations. Registered extension commands run immediately and
cannot be queued. [Prompts and queues](prompts.md) gives ordering and refusal
semantics.

The listener above matches `AgentSessionEvent::Agent`, then
`AgentEvent::MessageUpdate`, then `AssistantMessageEvent::TextDelta`. The same
agent stream carries thinking deltas, message starts/ends, tool execution and
turn/run boundaries. Session events also report queues, compaction and retries.
Read finished messages for authoritative history; a partial delta is a rendering
observation. Do not retain lock guards or `RefCell` borrows across callbacks or
awaits.

The check waits for the session's `AgentEnd` notification before inspecting the
collected text. This observes delivery to the session listener, not only the
agent becoming idle. It then calls the returned unsubscribe operation. A
callback should retain output state rather than the session, to avoid an
ownership cycle. See [session observations](lifecycle.md) and
[session events](session-operations.md#agent-events).

For shutdown, stop accepting input, await or abort active work, remove host
subscriptions, then dispose. `AgentSession::dispose` releases resources and
invalidates extension contexts but does not abort a run. When a replacement
runtime owns the session, use its disposal operation so extension shutdown and
host teardown run too. Flush retained settings before ending the executor.

## Report construction problems

A successful `CreateAgentSessionResult` can still carry problems:
`model_fallback_message` explains a missing or substituted model, and
`extensions_result.errors` contains extension load failures. Read skill, prompt
and theme diagnostics from the loader's results too. The direct factory returns
fatal failures as `DiagnosticErrorInfo`.

When creating services first, report their `diagnostics` before prompting.
`CreateAgentSessionRuntimeResult` carries those diagnostics beside the session,
services, loaded extensions and model fallback message; the runtime exposes the
current diagnostics after replacement. Settings and credentials retain their
own error queues. The host decides which diagnostics prevent a run and where
to show them; do not send them to a protocol output stream as ordinary text.

## Rebuild directory-bound services on replacement

For a host with new, resume, fork or import actions, use
`create_agent_session_runtime` to manage session replacement.
The factory receives the effective directory, history and start event. Capture
fixed process inputs in the factory; inside each call:

1. Create services with `create_agent_session_services` for that directory.
2. Call `create_agent_session_from_services` with the supplied history and start event.
3. Return the session, services, diagnostics, extensions and fallback message.

Use `set_before_session_invalidate` to release the old host subscription and
`set_rebind_session` to install the new one. Reacquire `runtime.session()` after
replacement; a previously cloned `Rc` still names the disposed session. Bind
extensions again on the replacement with the host's `ExtensionBindings` so it
receives `session_start` and the correct UI and command operations. Resource
loaders, tool directories and settings must belong to the effective directory,
not the one captured when the host first started.

A cancelled replacement keeps the current session. A construction failure after
teardown restores nothing; do not continue using old extension contexts. The
[replacement rules](#replacing-the-session) below describe each operation and
the point where teardown can fail.

## Compose a frontend

The application root exports session construction and replacement records,
resource loaders, credentials, catalog and settings types, history records and
tool definitions. It does not export every frontend runner as one SDK facade.
Use the owning crate for presentation:

| Host | Entry | Host supplies |
| --- | --- | --- |
| Interactive terminal | `maestro_chat::InteractiveMode::new` | Session runtime, mode options, terminal/runtime host, themes and footer effects; then initialize and run the mode. |
| One-shot text or JSON | `maestro_cli::run_print_mode` | Session runtime and `PrintModeOptions`; the runner presents output and handles cleanup. |
| Standard-stream RPC | `maestro_rpc::run_rpc_mode` | Session runtime and native mode facilities; the adapter owns standard streams and process exit. |
| Caller-owned connection | `maestro_app::run_rpc_connection` | Typed options, a line reader and writer, and optional session/workspace services; the runner returns instead of exiting. |

Follow [interactive mode](../chat/interactive-mode.md), [print mode](../print.md)
and [RPC](../rpc.md) for their full construction and lifecycle contracts. Reuse
those runners rather than starting another agent beside the supplied runtime.
For a child-process host, use the [RPC client](../client.md) instead of assembling
an in-process frontend.

The [SDK example index](../../examples/sdk/README.md) records what ships. The
thirteen standalone embedding examples are owned by
[the executable SDK examples issue](https://github.com/Orchestration-Maestro/maestro/issues/227);
they are not runnable files in that folder yet. The controlled program in this
guide runs as an application doctest.

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
  `AgentSessionServices` inherit the selection those services were created with.
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

`create_agent_session_from_services` creates a session over services that
already exist: their directories, credentials, catalog, preferences and loader.

## Requests

Each provider request takes the catalog's key and headers for the model; a
credential failure fails the request with its message. The provider retry
settings supply the timeout, the retry limit and the longest retry delay where
the request sets none, and the `httpIdleTimeoutMs` setting supplies the timeout
when neither the request nor the provider retry settings set one; its `0` or
`disabled` asks for the largest timeout a request carries, `i32::MAX` milliseconds, so
every provider that reads the timeout stops limiting the wait. Headers merge in order: attribution headers, then the
credential headers, then the request's own, later names replacing earlier
values.

While install telemetry is on, requests to `openrouter` or an `openrouter.ai`
address carry `HTTP-Referer: https://github.com/Orchestration-Maestro/maestro`,
`X-OpenRouter-Title: maestro` and `X-OpenRouter-Categories: cli-agent`. Requests
to `nvidia` or an `integrate.api.nvidia.com` address carry
`X-BILLING-INVOKE-ORIGIN: maestro`. Requests to Cloudflare's providers or to
`api.cloudflare.com` and `gateway.ai.cloudflare.com` addresses carry
`User-Agent: maestro-coding-agent`. Requests to `opencode`, `opencode-go` or an
`opencode.ai` address that carry a session ID also send `x-opencode-session` and
`x-opencode-client: maestro`, whatever the telemetry setting.

An address matches by its parsed host, ignoring case. A path or query that only
names a host, such as `https://proxy.example/api.cloudflare.com/v1`, matches
nothing.

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
sends the start event and passes the skill, prompt and theme paths extensions
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

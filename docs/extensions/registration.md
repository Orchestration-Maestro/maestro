# Owned extension registration

`maestro-extensions` registers tools, typed events, commands, flags, shortcuts and message renderers on
caller-supplied extension owners, and creates the base [contexts](#contexts) their
callbacks receive. It does not load extension artifacts or own application policy.
The caller supplies provenance, an existing event bus, the served session,
effective keybindings and the live theme the headless UI fallback reports; the
[UI capabilities](ui.md) guide covers UI hosts and renderers. The loader or runtime retains a strong extension
owner. API clones hold weak owner handles, so callbacks may capture them without
keeping a discarded extension alive. After the last strong owner is released,
registration and facade flag reads return the stale-context error.

## Register and query

```rust
use maestro_catalog::ModelRegistry;
use maestro_credentials::AuthStorage;
use maestro_extensions::{create_event_bus, Extension, ExtensionAPI, ExtensionRunner, SourceInfo};
use maestro_extensions::runner::ExtensionRunnerSession;
use maestro_extensions::types::{
    RegisteredCommandOptions, ExtensionFlagOptions,
    ExtensionShortcutOptions, FlagType, FlagValue,
};
use maestro_models::SharedHandle;
use maestro_session::SessionManager;
use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};

# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
let source: SourceInfo = serde_json::from_value(serde_json::json!({
    "path": "/extensions/example", "source": "cli",
    "scope": "temporary", "origin": "top-level"
}))?;
let owner = Arc::new(RwLock::new(Extension::<maestro_models::Threaded>::new(
    source.path.clone(), "/resolved/example".into(), Arc::new(RwLock::new(source)),
)));
let owners = Arc::new(RwLock::new(vec![Arc::clone(&owner)]));
let runtime = maestro_extensions::create_extension_runtime::<maestro_models::Threaded>();
let bus = create_event_bus(
    Arc::new(|tail| { tokio::spawn(tail); }),
    Arc::new(|line| eprint!("{line}")),
);
let api = ExtensionAPI::new(&owner, runtime.clone(), bus.for_extension(), "/workspace".into());
api.register_command("hello".into(), RegisteredCommandOptions {
    description: Some("Say hello".into()),
    get_argument_completions: None,
    handler: Arc::new(|args, _ctx| Box::pin(async move {
        assert_eq!(args, "raw arguments");
        Ok(())
    })),
})?;
api.register_flag("quiet".into(), ExtensionFlagOptions {
    description: None, r#type: FlagType::Boolean,
    default: Some(FlagValue::Boolean(false)),
})?;
api.register_shortcut("ctrl+x".into(), ExtensionShortcutOptions {
    description: None,
    handler: Arc::new(|ctx| {
        let cwd = ctx.cwd().map(str::to_owned);
        Box::pin(async move { assert_eq!(cwd?, "/work"); Ok(()) })
    }),
})?;
let auth = SharedHandle::new(AuthStorage::in_memory(Default::default()));
# struct NoCommands;
# impl maestro_credentials::ConfigValueOperations for NoCommands {
#     fn execute(&self, _command: &str) -> Option<Vec<u8>> {
#         None
#     }
# }
# let no_commands = SharedHandle::new(NoCommands);
let session = ExtensionRunnerSession {
    cwd: "/work".into(),
    session_manager: Rc::new(RefCell::new(SessionManager::in_memory(Some("/work"))?)),
    model_registry: maestro_models::SharedHandle::new(ModelRegistry::in_memory(auth, no_commands)?),
};
let runner = ExtensionRunner::<maestro_models::Threaded>::new(
    owners, runtime, session, Arc::new(|line| eprint!("{line}")),
    maestro_theme::LiveTheme::default(),
);
assert_eq!(api.get_flag("quiet")?, Some(FlagValue::Boolean(false)));
assert!(runner.get_shortcuts(&vec![]).contains_key("ctrl+x"));
let command = runner.get_command("hello").ok_or("command missing")?;
let executor = tokio::runtime::Builder::new_current_thread().build()?;
executor.block_on((command.handler)("raw arguments".into(), runner.create_command_context()))?;
let shortcuts = runner.get_shortcuts(&vec![]);
let shortcut = Arc::clone(&shortcuts["ctrl+x"].read().map_err(|e| e.to_string())?.handler);
executor.block_on(shortcut(runner.create_context()))?;
# Ok(())
# }
```

## Local callbacks

Registration records, the facade and the runner take an execution family, which
defaults to `maestro_models::Threaded`, thread-safe on native targets; on browser
targets it uses `Rc` and callbacks without `Send + Sync`. The family selects only
the callback representation: the runner itself is always local. With `maestro_models::Local`,
command, shortcut, completion and event callbacks use `Rc` and may hold
single-thread values such as terminal components. Both families share one
registration owner, one query algorithm and one event bus implementation.
`create_event_bus_for::<Local>` creates the bus for local callbacks; its spawner
must admit local tails, for example through a Tokio local task set.

```rust
use maestro_extensions::types::{RegisteredCommandOptions};
use maestro_extensions::{create_event_bus_for, Extension, ExtensionAPI, ExtensionRunner, SourceInfo};
use maestro_models::Local;
use maestro_tui::Input;
use std::{rc::Rc, sync::{Arc, RwLock}};

fn local_callback_registration_example() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let source: SourceInfo = serde_json::from_value(serde_json::json!({
        "path": "/extensions/rename", "source": "cli",
        "scope": "temporary", "origin": "top-level"
    }))?;
    let owner = Rc::new(RwLock::new(Extension::<Local>::new(
        source.path.clone(), "/resolved/rename".into(), Arc::new(RwLock::new(source)),
    )));
    let owners = Rc::new(RwLock::new(vec![Rc::clone(&owner)]));
    let runtime = maestro_extensions::create_extension_runtime::<maestro_models::Local>();
    let bus = create_event_bus_for::<Local>(
        Rc::new(|tail| { tokio::task::spawn_local(tail); }),
        Rc::new(|line| eprint!("{line}")),
    );
    let api = ExtensionAPI::new(&owner, runtime.clone(), bus.for_extension(), "/workspace".into());
    let input = Rc::new(Input::new());
    let captured_input = Rc::clone(&input);
    api.register_command("rename".into(), RegisteredCommandOptions {
        description: Some("Replace the input text".into()),
        get_argument_completions: None,
        handler: Rc::new(move |args, ctx| {
            let input = Rc::clone(&captured_input);
            Box::pin(async move {
                assert_eq!(ctx.cwd()?, "/work");
                input.set_value(args);
                Ok(())
            })
        }),
    })?;
#   let auth = maestro_models::SharedHandle::new(maestro_credentials::AuthStorage::in_memory(Default::default()));
#   struct NoCommands;
#   impl maestro_credentials::ConfigValueOperations for NoCommands {
#       fn execute(&self, _command: &str) -> Option<Vec<u8>> {
#           None
#       }
#   }
#   let no_commands = maestro_models::SharedHandle::new(NoCommands);
#   let session = maestro_extensions::runner::ExtensionRunnerSession {
#       cwd: "/work".into(),
#       session_manager: Rc::new(std::cell::RefCell::new(maestro_session::SessionManager::in_memory(None)?)),
#       model_registry: maestro_models::SharedHandle::new(maestro_catalog::ModelRegistry::in_memory(auth, no_commands)?),
#   };
    let runner = ExtensionRunner::<Local>::new(
        owners, runtime, session, Rc::new(|line| eprint!("{line}")),
        maestro_theme::LiveTheme::default(),
    );
    let command = runner.get_command("rename").ok_or("command missing")?;
    let executor = tokio::runtime::Builder::new_current_thread().build()?;
    executor.block_on((command.handler)("draft.md".into(), runner.create_command_context()))?;
    assert_eq!(input.get_value(), "draft.md");
    Ok(())
}
# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
# local_callback_registration_example()
# }
```

The family is a type parameter resolved through associated pointer types, so
the compiler cannot infer it from a callback alone. Name it where nothing else
fixes it: a struct literal bound before registration
(`RegisteredCommandOptions::<maestro_models::Threaded> { .. }`) or a runner built from a collection
(`ExtensionRunner::<maestro_models::Threaded>::new`). Literals passed directly to a facade method
infer the facade's family.

## Retained message renderer calls

`ExtensionAPI::register_message_renderer` stores a typed renderer beside its
class, keeping the original slot on replacement and retaining the registering
bus with the callback. `ExtensionRunner::get_message_renderer` returns the raw
callback of the first owner that registered the class; a different details type
on that owner is absent and later owners are not consulted.
`ExtensionRunner::get_message_renderer_call` selects the same owner and returns a
`MessageRendererCall`, which keeps the callback and that bus together.

`MessageRendererCall::new` binds a supplied renderer to an existing bus for
callers that hold a renderer already, and `MessageRendererCall::call` invokes it
once through the ordinary callback frame: same-extension listeners run before the
callback returns and foreign listeners afterwards. A retained call keeps working
after its registration is replaced or removed, and releases the callback and bus
with its last handle. It never captures the runner. Clone shares both owners
without requiring details that can be cloned.

```rust
use maestro_extensions::{MessageRenderOptions, MessageRenderer, MessageRendererCall};
use maestro_extensions::create_event_bus;
use maestro_models::{Execution, Threaded, UserContent};
use maestro_session::Presence;
use maestro_session::messages::CustomMessage;
use std::sync::Arc;

# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
let bus = create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
let renderer: MessageRenderer<u8, Threaded> =
    Arc::new(|_message, _options, _theme, _emitter| Ok(None));
let call = MessageRendererCall::<u8, Threaded>::new(renderer, (*bus).clone());
let outcome = call.call(
    Threaded::share(CustomMessage {
        custom_type: "note".into(),
        content: UserContent::Text("body".into()),
        display: true,
        details: Presence::Missing,
        timestamp: 0.0,
    }),
    MessageRenderOptions { expanded: false },
    maestro_theme::LiveTheme::default(),
)?;
assert!(outcome.is_none());
# Ok(())
# }
```

## Commands

Same-owner replacement keeps the original insertion position. Queries read the
current load and registration order. Duplicate base names receive one-based
suffixes, such as `hello:1` and `hello:2`. If an invocation name is already taken,
its suffix advances until free; future literal names are not pre-reserved.
Lookup uses the resolved invocation name, not a duplicated unsuffixed base name.
Listing commands publishes a fresh empty command-diagnostic collection.

Argument completion is optional. A supplied callback returns `None`, an empty
candidate list or ordered candidates with value, label and optional description.
Arguments and completion prefixes are passed unchanged; callback failures remain
catchable errors. Resolved commands copy their top-level fields but retain the
registered source object and callbacks.

## Flags and activity

Flag metadata keeps the first owner across extensions. Defaults seed only missing
shared values, without validating the default against the declared kind. False
and empty strings remain present. An owner can read only names it registered;
the runner can set any name. Value snapshots are independent ordered maps, while
returned flag records retain their owners.

Invalidation keeps the first message, including an empty string. A stale shared
runtime state makes registration and facade flag reads return that error before
effects. `ExtensionRunner::invalidate` also invalidates the runtime, and every
guarded operation of that runner's contexts then fails with the runner's first
message. A runner created later over the same runtime keeps its contexts
active. Runner queries and its flag setter remain available. Omitting the
message uses guidance about stale contexts after session replacement or reload.

## Shortcuts and diagnostics

Matching lowercases supplied keys without changing raw registration text or the
captured owner path. Effective bindings for editor-global reserved actions block
overrides. Other built-in collisions warn but allow overrides. Later extension
registrations replace normalized-key collisions in the first key's slot.

Each shortcut query publishes fresh diagnostics; an earlier retained collection
is not cleared. An installed UI context suppresses only warning output, not
diagnostics or selection; with the headless fallback each warning is reported. A warning reporter receives one complete line, including its final
newline, outside registration locks. Flag and shortcut paths are captured when
registered; changing an owner's current path does not rewrite them.

## Contexts

`ExtensionRunner::create_context` returns the `ExtensionContext` callbacks
receive. It shares its runner's state and reads it on each call: the UI context,
the working directory, session history and model catalog of the
`ExtensionRunnerSession` given to `ExtensionRunner::new`, and the application
operations last given to `bind_core`. The model reader is the exception: a
context keeps the `get_model` operation that was bound when it was created, and
calls it on every access. Before-agent hook contexts additionally retain their invocation's effective prompt; see [native hooks](hooks.md). Before any binding, a context reports no model, idle,
no cancellation, no pending messages, no usage and an empty system prompt, and
its abort, shutdown and compaction requests do nothing.

Each operation returns its bound operation's value or error unchanged.
`compact` passes its options, callbacks included, unchanged and returns without
waiting for compaction. Once the runner is invalidated, every operation returns
the stale error before reading state or calling an operation. Values returned
earlier (UI context, session history, catalog, cancellation and model) keep
working; nothing is revoked or retargeted. Creating contexts, the runner's
queries and `ExtensionRunner::shutdown`, which calls the bound shutdown
operation, stay available.

A context is a single-thread handle. A thread-safe command or shortcut callback receives it
by value and returns a `Send` future, so it reads the context before its first
suspension; a `Local` callback may keep it across suspensions.

```rust
use maestro_catalog::ModelRegistry;
use maestro_credentials::AuthStorage;
use maestro_extensions::runner::ExtensionRunnerSession;
use maestro_extensions::{ExtensionContextActions, ExtensionRunner};
use maestro_models::SharedHandle;
use maestro_session::SessionManager;
use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};

# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
# const STALE: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";
let auth = SharedHandle::new(AuthStorage::in_memory(Default::default()));
# struct NoCommands;
# impl maestro_credentials::ConfigValueOperations for NoCommands {
#     fn execute(&self, _command: &str) -> Option<Vec<u8>> {
#         None
#     }
# }
# let no_commands = SharedHandle::new(NoCommands);
let session = ExtensionRunnerSession {
    cwd: "/work".into(),
    session_manager: Rc::new(RefCell::new(SessionManager::in_memory(Some("/work"))?)),
    model_registry: maestro_models::SharedHandle::new(ModelRegistry::in_memory(auth, no_commands)?),
};
let runner = ExtensionRunner::<maestro_models::Threaded>::new(
    Arc::new(RwLock::new(Vec::new())), maestro_extensions::create_extension_runtime::<maestro_models::Threaded>(), session,
    Arc::new(|line| eprint!("{line}")), maestro_theme::LiveTheme::default(),
);
let ctx = runner.create_context();
assert!(ctx.is_idle()?);
runner.bind_core(<maestro_models::Threaded as maestro_extensions::ExtensionExecution>::unbound_actions(), ExtensionContextActions {
    is_idle: Rc::new(|| Ok(false)),
    ..ExtensionContextActions::default()
}, maestro_extensions::types::ProviderActions::default()).unwrap();
assert!(!ctx.is_idle()?);
let history = ctx.session_manager()?;
runner.invalidate(None);
assert_eq!(ctx.cwd().unwrap_err().to_string(), STALE);
assert_eq!(history.borrow().get_cwd(), "/work");
# Ok(())
# }
```

### Command and replacement contexts

Commands receive `ExtensionCommandContext`, the base context plus six operations:
`wait_for_idle`, `new_session`, `fork`, `navigate_tree`, `switch_session` and `reload`.
The six are `CommandContextOperations`, which a runtime owns;
`ExtensionCommandContextActions::new` wraps them for a runner to bind. A session binds
the value its application runtime builds, so a caller asks the runtime instead of
supplying a second policy. `bind_command_context(Some(actions))` replaces these
independently of `bind_core`; `None` restores no-op waits and reloads and uncancelled
session-change results. Defaults never run supplied setup or continuation callbacks.

Each call checks activity, selects the current operation and starts it immediately,
before its returned local future is polled. A later rebind or invalidation does not
change work already started. Options and errors pass through unchanged. Command
contexts capture their model reader at creation and keep all other base reads lazy.
Like base contexts, they are single-thread handles: use local command handlers to
keep one across suspension. Reload can finish the old command frame, but later
context calls fail with the stale error.

`ReplacedSessionContext` combines a replacement runner's command context with the
creating session's custom- and user-message operations. Those two operations are
unguarded and keep reaching their supplied session even after runner invalidation;
base and command members still apply the guard. Application operations, not the
context, run `setup` and `with_session` callbacks and create replacement sessions.

```rust
use maestro_extensions::{
    CommandContextOperations, ExtensionCommandContextActions, ExtensionRunner, ReplacedSessionContext,
};
use maestro_extensions::types::{NewSessionCommandOptions, SessionChangeResult};
use maestro_extensions::runner::ExtensionRunnerSession;
use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};
# fn no_op_operations() -> CommandContextOperations {
#     let uncancelled = || Box::pin(std::future::ready(Ok(SessionChangeResult { cancelled: false })));
#     CommandContextOperations {
#         wait_for_idle: Rc::new(|| Box::pin(std::future::ready(Ok(())))),
#         new_session: Rc::new(move |_| uncancelled()),
#         fork: Rc::new(move |_, _| uncancelled()),
#         navigate_tree: Rc::new(move |_, _| uncancelled()),
#         switch_session: Rc::new(move |_, _| uncancelled()),
#         reload: Rc::new(|| Box::pin(std::future::ready(Ok(())))),
#     }
# }
# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
# let auth = maestro_models::SharedHandle::new(maestro_credentials::AuthStorage::in_memory(Default::default()));
# struct NoCommands;
# impl maestro_credentials::ConfigValueOperations for NoCommands {
#     fn execute(&self, _command: &str) -> Option<Vec<u8>> {
#         None
#     }
# }
# let no_commands = maestro_models::SharedHandle::new(NoCommands);
# let session = ExtensionRunnerSession {
#     cwd: "/work".into(),
#     session_manager: Rc::new(RefCell::new(maestro_session::SessionManager::in_memory(None)?)),
#     model_registry: maestro_models::SharedHandle::new(maestro_catalog::ModelRegistry::in_memory(auth, no_commands)?),
# };
let runner = ExtensionRunner::<maestro_models::Threaded>::new(
    Arc::new(RwLock::new(Vec::new())), maestro_extensions::create_extension_runtime::<maestro_models::Threaded>(), session,
    Arc::new(|_| {}), maestro_theme::LiveTheme::default(),
);
runner.bind_command_context(Some(ExtensionCommandContextActions::new(
    CommandContextOperations {
        new_session: Rc::new(|options| {
            assert_eq!(options.unwrap().parent_session.as_deref(), Some("/parent.jsonl"));
            Box::pin(async { Ok(SessionChangeResult { cancelled: true }) })
        }),
        ..no_op_operations()
    },
)));
let ctx = runner.create_command_context();
let executor = tokio::runtime::Builder::new_current_thread().build()?;
executor.block_on(async {
    assert!(ctx.new_session(Some(NewSessionCommandOptions {
        parent_session: Some("/parent.jsonl".into()), ..Default::default()
    })).await?.cancelled);
    runner.bind_command_context(None);
    assert!(!ctx.new_session(None).await?.cancelled);
    let sent = Rc::new(RefCell::new(Vec::new()));
    let output = Rc::clone(&sent);
    let replacement = ReplacedSessionContext::new(ctx,
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
        Rc::new(move |content, _| {
            output.borrow_mut().push(content);
            Box::pin(async { Ok(()) })
        }),
    );
    runner.invalidate(None);
    assert!(replacement.cwd().is_err());
    replacement.send_user_message(maestro_models::UserContent::Text("hello".into()), None).await?;
    assert!(matches!(sent.borrow().as_slice(), [maestro_models::UserContent::Text(text)] if text == "hello"));
    Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
})?;
# Ok(())
# }
```

See [extension context lifecycle](lifecycle.md) for retained handles, replacement
callbacks and the application/runtime boundaries. These functions illustrate
local command handlers; their bodies compile here but need application-bound
operations to perform replacement or reload.

### Continue with the fresh context

Capture plain data and send through the context supplied to `with_session`.

```rust
use maestro_extensions::{EventError, ExtensionCommandContext};
use maestro_extensions::types::NewSessionCommandOptions;
use maestro_models::UserContent;
use std::rc::Rc;

pub async fn safe_handoff(ctx: ExtensionCommandContext) -> Result<(), EventError> {
    let kickoff = String::from("Continue from the replacement session");
    ctx.new_session(Some(NewSessionCommandOptions {
        with_session: Some(Rc::new(move |fresh| {
            let kickoff = kickoff.clone();
            Box::pin(async move {
                fresh.send_user_message(UserContent::Text(kickoff), None).await
            })
        })),
        ..Default::default()
    })).await?;
    Ok(())
}
# fn main() {}
```

### Avoid captured contexts

This example is deliberately incorrect after successful replacement: the old
context returns the stale error. A cancelled replacement leaves it active.

```rust
use maestro_extensions::{EventError, ExtensionCommandContext};

pub async fn captured_context_is_not_fresh(ctx: ExtensionCommandContext) -> Result<(), EventError> {
    let old = ctx.clone();
    let result = ctx.new_session(None).await?;
    if !result.cancelled {
        old.cwd()?;
    }
    Ok(())
}
# fn main() {}
```

### Write history during setup

The application operation invokes setup with writable session history.

```rust
use maestro_extensions::{EventError, ExtensionCommandContext};
use maestro_extensions::types::NewSessionCommandOptions;
use std::rc::Rc;

pub async fn setup_history(ctx: ExtensionCommandContext) -> Result<(), EventError> {
    ctx.new_session(Some(NewSessionCommandOptions {
        setup: Some(Rc::new(|history| Box::pin(async move {
            history.borrow_mut().append_session_info("handoff")?;
            Ok(())
        }))),
        ..Default::default()
    })).await?;
    Ok(())
}
# fn main() {}
```

### Reload and return

Do not use the old context after awaiting reload.

```rust
use maestro_extensions::{EventError, ExtensionCommandContext};

pub async fn reload_and_return(ctx: ExtensionCommandContext) -> Result<(), EventError> {
    ctx.reload().await
}
# fn main() {}
```

## Actions and provider binding

Create one `create_extension_runtime::<Local>()` or
`create_extension_runtime::<Threaded>()` for the facade and runner. Supply the
facade's creation directory explicitly; native `exec` delegates to the tools
operation with that directory unless a call overrides it.

`ExtensionExecution::unbound_actions()` supplies loading-time errors, except for
its inert tool refresh. `bind_core(actions, context_actions, provider_actions)`
installs action and base-context callbacks before flushing queued providers.
Command binding remains independent. Facades check their existing owner/activity
guard; direct runtime action access is unguarded. A started model-selection future
retains the selected operation even after rebinding.

Provider configuration is the catalog's complete input record, including stream
and account callbacks. Before binding, registrations retain insertion order and
originating paths; unregister removes every pending match. Flushes observe later
config edits, additions and cancellations. Failures reach attributed error
listeners and normally do not stop later registrations. If a listener fails,
binding returns that error: successful registrations are consumed, while failed
and unattempted work remains for the next bind. After a completed flush, each
provider operation independently uses its supplied override or the same direct
shared catalog returned by contexts.

`on_error` deduplicates callback identity and returns an explicit remover.
Dropping the remover does not unsubscribe. Delivery observes listener additions
and removals live; the first listener error stops delivery. `emit_error` accepts
supplied fields without manufacturing stack text and remains usable when stale.
Local actions/futures may capture local values; native Threaded actions are
`Send + Sync` with `Send` futures. No runtime guard is retained while calling an
application callback.

```rust
use maestro_extensions::{ExtensionActions, ExtensionExecution, types::ProviderActions};
use maestro_models::Local;
use std::rc::Rc;
# fn bind(runner: &maestro_extensions::ExtensionRunner<Local>) -> Result<(), maestro_extensions::EventError> {
let mut actions: ExtensionActions<Local> = Local::unbound_actions();
actions.get_session_name = Rc::new(|| Ok(Some("bound session".into())));
let providers = ProviderActions::<Local> {
    register_provider: Some(Rc::new(|name, config| {
        // Application-supplied registration owns its policy.
        assert_eq!(name, "example");
        assert!(config.read().map_err(|error| error.to_string())?.models.is_none());
        Ok(())
    })),
    unregister_provider: None,
};
runner.bind_core(actions, Default::default(), providers)?;
# Ok(())
# }
```

## Retained tools and typed events

`ExtensionAPI::register_tool` retains owned or shared mutable definitions alongside
provenance in the same ordered owner. Replacement keeps its slot, then calls the
current refresh operation after releasing the owner guard. A refresh failure does
not undo registration; the loading runtime's refresh is inert.

`get_all_registered_tools` deduplicates current definition names in load order.
`get_tool_definition::<P, D, S>` instead searches stored registration keys and
returns the original typed handle. A type mismatch on the first owner is absent;
it does not select a later owner. Different parameter, details and renderer-state
types coexist in one owner. `RegisteredTool::metadata` and
`RegisteredTool::renderer_binding` read a retained definition's current metadata
and bound renderers for callers that know only the execution family.

The tools-owned wrapper snapshots declaration/preparation/mode metadata when
created. Each execution selects the current retained execute callback before
creating a fresh concrete runner context. A context keeps the model reader bound
when it was created. Both ownership families invoke registered tools locally;
Threaded captures stay Send + Sync, while a registered invocation may retain its
local context across an await. Standalone SDK invocation remains Threaded by
default. Renderer state remains local and is projected by `ToolRendererBinding`.
Typed result renderers decode details through `from_definition`; raw renderers and
Serialize-only details use `RegisteredToolInput::from_raw` without a decoder.

A parameter schema built by a schema builder carries the kind of each node its author
declared, beside the ordinary JSON Schema value. Argument preparation converts those
nodes before the plain conversion pass; a schema written as ordinary JSON has no
declared kinds and is converted as JSON alone. Provider payloads and saved JSON drop
the declared kinds.

```rust
use maestro_extensions::{ExtensionContext, ExtensionFactory, load_extension_from_factory};
use maestro_models::{Local, Tool};
use maestro_tools::{ToolDefinitionRecord, tool_definition_wrapper::ToolDefinitionInput};
use serde_json::{Value, json};
use std::{rc::Rc, sync::RwLock};

# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
# let executor = tokio::runtime::Builder::new_current_thread().build()?;
# executor.block_on(async {
let factory: ExtensionFactory<Local> = Rc::new(|api| {
    let tool: ToolDefinitionRecord<ExtensionContext, Value, Value, (), Local, Local> =
        ToolDefinitionRecord {
            definition: Tool { name: "echo".into(), description: "Echo arguments".into(),
                parameters: json!({"type":"object"}).into() },
            label: "Echo".into(), prompt_snippet: None, prompt_guidelines: None,
            render_shell: None, prepare_arguments: None, execution_mode: None,
            execute: Rc::new(|_, params, _, _, ctx| Box::pin(async move {
                let context = ctx.ok_or_else(|| maestro_models::DiagnosticErrorInfo::error("missing context"))?;
                let _cwd = context.cwd().map(str::to_owned)
                    .map_err(|error| maestro_models::DiagnosticErrorInfo::error(error.to_string()))?;
                Ok(maestro_agent::AgentToolResult { content: vec![], details: Some(params), terminate: None, usage: None })
            })), render_call: None, render_result: None,
        };
    let retained = Rc::new(RwLock::new(tool));
    let registered = api.register_tool(ToolDefinitionInput::<_, _, _, _, Local, Local>::Retained(retained));
    Box::pin(async move { registered })
});
let bus = maestro_extensions::create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
let owner = load_extension_from_factory::<Local>(factory, "/work".into(), bus.for_extension(),
    maestro_extensions::create_extension_runtime::<Local>(), None).await?;
assert!(owner.read().map_err(|error| error.to_string())?.tools.contains_key("echo"));
# Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
# })
# }
```

`on<V, R>` stores typed callbacks under unchanged string event names, including
unknown and empty names; repeated callback registrations remain separate entries.
The [hook runner](hooks.md) dispatches them. Event/result schemas and assembled prompt records
belong to the shared request owner, re-exported by models and specialized with
native domain handles or the guest's existing carriers. `BuildSystemPromptOptions`
is also exported at the extension root for constructing prompt operands. The guest adapter alone
snapshots stream observations and encodes capability resources.

Local definitions retain executable parameter schemas (`ParameterSchema<SchemaCode>`).
Registration, enumeration and re-registration keep their author callbacks alive;
validation calls them after argument preparation and before typed execution decoding.
Default threaded definitions and model request contexts remain data-only.

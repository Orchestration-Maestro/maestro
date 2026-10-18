# Owned extension registration

`maestro-extensions` registers commands, flags, shortcuts and message renderers on
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
let auth = Arc::new(AuthStorage::in_memory(Default::default()));
let session = ExtensionRunnerSession {
    cwd: "/work".into(),
    session_manager: Rc::new(RefCell::new(SessionManager::in_memory(Some("/work"))?)),
    model_registry: maestro_models::SharedHandle::new(ModelRegistry::in_memory(auth)?),
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
#   let auth = Arc::new(maestro_credentials::AuthStorage::in_memory(Default::default()));
#   let session = maestro_extensions::runner::ExtensionRunnerSession {
#       cwd: "/work".into(),
#       session_manager: Rc::new(std::cell::RefCell::new(maestro_session::SessionManager::in_memory(None)?)),
#       model_registry: maestro_models::SharedHandle::new(maestro_catalog::ModelRegistry::in_memory(auth)?),
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

Invalidation, through `ExtensionRunner::invalidate` or the shared runtime state,
retains the first message, including an empty string. Registration, facade flag
reads and every guarded context operation then return that error before effects. Runner
queries and its flag setter remain available. Omitting the message uses guidance
about stale contexts after session replacement or reload.

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
calls it on every access. Before any binding, a context reports no model, idle,
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
use maestro_session::SessionManager;
use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};

# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
# const STALE: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";
let auth = Arc::new(AuthStorage::in_memory(Default::default()));
let session = ExtensionRunnerSession {
    cwd: "/work".into(),
    session_manager: Rc::new(RefCell::new(SessionManager::in_memory(Some("/work"))?)),
    model_registry: maestro_models::SharedHandle::new(ModelRegistry::in_memory(auth)?),
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
`bind_command_context(Some(actions))` replaces these independently of `bind_core`;
`None` restores no-op waits and reloads and uncancelled session-change results.
Defaults never run supplied setup or continuation callbacks.

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
use maestro_extensions::{ExtensionCommandContextActions, ExtensionRunner, ReplacedSessionContext};
use maestro_extensions::types::{NewSessionCommandOptions, SessionChangeResult};
use maestro_extensions::runner::ExtensionRunnerSession;
use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};
# fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
# let auth = Arc::new(maestro_credentials::AuthStorage::in_memory(Default::default()));
# let session = ExtensionRunnerSession {
#     cwd: "/work".into(),
#     session_manager: Rc::new(RefCell::new(maestro_session::SessionManager::in_memory(None)?)),
#     model_registry: maestro_models::SharedHandle::new(maestro_catalog::ModelRegistry::in_memory(auth)?),
# };
let runner = ExtensionRunner::<maestro_models::Threaded>::new(
    Arc::new(RwLock::new(Vec::new())), maestro_extensions::create_extension_runtime::<maestro_models::Threaded>(), session,
    Arc::new(|_| {}), maestro_theme::LiveTheme::default(),
);
runner.bind_command_context(Some(ExtensionCommandContextActions {
    new_session: Rc::new(|options| {
        assert_eq!(options.unwrap().parent_session.as_deref(), Some("/parent.jsonl"));
        Box::pin(async { Ok(SessionChangeResult { cancelled: true }) })
    }),
    ..Default::default()
}));
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

//! Live guarded base contexts over the runner's bound capabilities.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[allow(dead_code, reason = "Shared local fixture.")]
#[path = "support/local.rs"]
mod local_support;
#[path = "support/model.rs"]
mod model_support;
#[path = "support/ui_host.rs"]
mod ui_host;
use ui_host::Named;

use local_support::{LocalBus, STALE, loaded, local, session::runner_session};
use maestro_catalog::ModelRegistry;
use maestro_extensions::types::{
    ExtensionCollection, ExtensionFlagOptions, ExtensionRuntimeState, ExtensionShortcutOptions,
    ExtensionUIContextPort, FlagType, RegisteredCommandOptions, ShortcutHandler,
};
use maestro_extensions::{
    CompactOptions, ContextUsage, EventError, Extension, ExtensionAPI, ExtensionContext,
    ExtensionContextActions, ExtensionRunner, create_event_bus,
};
use maestro_models::{Cancellation, Local, Model};
use maestro_session::SessionManager;
use maestro_theme::LiveTheme;
#[path = "support/error.rs"]
mod error_support;
use error_support::Marker;
use model_support::model;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};
use tokio::task::yield_now;

/// A local runner without registrations.
type Runner = ExtensionRunner<Local>;
/// A shared model handle.
type SharedModel = Arc<RwLock<Model>>;
/// Names of the bound operations called, in order.
type Log = Rc<RefCell<Vec<&'static str>>>;

/// A runner with the history and catalog the application keeps.
struct Served {
    /// The runner under test.
    runner: Runner,
    /// The writable history behind the runner's read-only handle.
    history: Rc<RefCell<SessionManager>>,
    /// The runner's catalog.
    catalog: Rc<RefCell<ModelRegistry>>,
}

/// Serves `cwd` for the supplied registrations and activity state.
fn served_with(
    cwd: &str,
    owners: &ExtensionCollection<Local>,
    runtime: &Arc<ExtensionRuntimeState>,
) -> Served {
    let (session, history) = runner_session(cwd);
    let catalog = Rc::clone(&session.model_registry);
    let runner = Runner::new(
        Rc::clone(owners),
        Arc::clone(runtime),
        session,
        Rc::new(|_| {}),
        LiveTheme::default(),
    );
    Served {
        runner,
        history,
        catalog,
    }
}

/// Serves `cwd` with no registrations and fresh activity state.
fn served(cwd: &str) -> Served {
    served_with(
        cwd,
        &Rc::new(RwLock::new(Vec::new())),
        &Arc::new(ExtensionRuntimeState::new()),
    )
}

/// Whether two optional model handles are the same model.
fn same_model(actual: Option<SharedModel>, expected: &SharedModel) -> bool {
    actual.is_some_and(|actual| Arc::ptr_eq(&actual, expected))
}

/// Bound operations that log their own name and return fixed values.
struct Recorder {
    /// Operation names in call order.
    log: Log,
    /// Compaction requests in call order.
    compacts: Rc<RefCell<Vec<Option<CompactOptions>>>>,
    /// The model the reader returns.
    model: SharedModel,
    /// The cancellation the signal reader returns.
    signal: Cancellation,
}
impl Recorder {
    /// A recorder with an empty log.
    fn new() -> Self {
        Self {
            log: Log::default(),
            compacts: Rc::default(),
            model: model("bound"),
            signal: Cancellation::new(),
        }
    }
    /// Takes the names logged since the last call.
    fn take(&self) -> Vec<&'static str> {
        std::mem::take(&mut *self.log.borrow_mut())
    }
    /// Operations returning this recorder's values after logging their names.
    fn actions(&self) -> ExtensionContextActions {
        let log = |name| {
            let log = Rc::clone(&self.log);
            move || log.borrow_mut().push(name)
        };
        let (model, signal, compacts) = (
            Arc::clone(&self.model),
            self.signal.clone(),
            Rc::clone(&self.compacts),
        );
        let (get_model, is_idle, get_signal) =
            (log("get_model"), log("is_idle"), log("get_signal"));
        let (abort, pending, shutdown) =
            (log("abort"), log("has_pending_messages"), log("shutdown"));
        let (usage, compact) = (log("get_context_usage"), log("compact"));
        let prompt = log("get_system_prompt");
        ExtensionContextActions {
            get_model: Rc::new(move || {
                get_model();
                Ok(Some(Arc::clone(&model)))
            }),
            is_idle: Rc::new(move || {
                is_idle();
                Ok(false)
            }),
            get_signal: Rc::new(move || {
                get_signal();
                Ok(Some(signal.clone()))
            }),
            abort: Rc::new(move || {
                abort();
                Ok(())
            }),
            has_pending_messages: Rc::new(move || {
                pending();
                Ok(true)
            }),
            shutdown: Rc::new(move || {
                shutdown();
                Ok(())
            }),
            get_context_usage: Rc::new(move || {
                usage();
                Ok(Some(ContextUsage {
                    tokens: None,
                    context_window: 200_000.0,
                    percent: None,
                }))
            }),
            compact: Rc::new(move |options| {
                compact();
                compacts.borrow_mut().push(options);
                Ok(())
            }),
            get_system_prompt: Rc::new(move || {
                prompt();
                Ok("bound prompt".to_owned())
            }),
        }
    }
}

/// A full compaction request whose callbacks reject and record any call.
fn full_compaction(called: &Rc<Cell<bool>>) -> CompactOptions {
    let (complete, error) = (Rc::clone(called), Rc::clone(called));
    CompactOptions {
        custom_instructions: Some("keep".into()),
        on_complete: Some(Rc::new(move |_| {
            complete.set(true);
            Err("unexpected completion".into())
        })),
        on_error: Some(Rc::new(move |_| {
            error.set(true);
            Err("unexpected failure".into())
        })),
    }
}

/// Calls every context operation, returning each error message in order.
fn errors(ctx: &ExtensionContext) -> Vec<Option<String>> {
    let message = |error: EventError| error.to_string();
    vec![
        ctx.ui().err().map(message),
        ctx.has_ui().err().map(message),
        ctx.cwd().err().map(message),
        ctx.session_manager().err().map(message),
        ctx.model_registry().err().map(message),
        ctx.model().err().map(message),
        ctx.is_idle().err().map(message),
        ctx.signal().err().map(message),
        ctx.abort().err().map(message),
        ctx.has_pending_messages().err().map(message),
        ctx.shutdown().err().map(message),
        ctx.get_context_usage().err().map(message),
        ctx.compact(None).err().map(message),
        ctx.get_system_prompt().err().map(message),
    ]
}

/// Bound operations reading the model and idle state.
fn reading(
    get_model: impl Fn() -> Option<SharedModel> + 'static,
    idle: bool,
) -> ExtensionContextActions {
    ExtensionContextActions {
        get_model: Rc::new(move || Ok(get_model())),
        is_idle: Rc::new(move || Ok(idle)),
        ..ExtensionContextActions::default()
    }
}

#[test]
fn existing_context_observes_later_bindings_except_captured_model_getter() {
    let s = served("/work");
    let early = s.runner.create_context();
    let (first, second, later) = (model("a1"), model("a2"), model("b"));
    let flip = Rc::new(Cell::new(false));
    let (a1, a2) = (Arc::clone(&first), Arc::clone(&second));
    s.runner.bind_core(reading(
        move || {
            Some(Arc::clone(if flip.replace(!flip.get()) {
                &a2
            } else {
                &a1
            }))
        },
        false,
    ));
    assert!(early.model().unwrap().is_none());
    assert!(!early.is_idle().unwrap());
    let ctx = s.runner.create_context();
    assert!(same_model(ctx.model().unwrap(), &first));
    assert!(same_model(ctx.model().unwrap(), &second));
    let b = Arc::clone(&later);
    s.runner
        .bind_core(reading(move || Some(Arc::clone(&b)), true));
    assert!(same_model(ctx.model().unwrap(), &first));
    assert!(ctx.is_idle().unwrap());
    assert!(early.model().unwrap().is_none());
    assert!(same_model(
        s.runner.create_context().model().unwrap(),
        &later
    ));
    let (ui, host) = Named::install("context");
    s.runner.set_ui_context(Some(ui));
    ctx.ui()
        .unwrap()
        .set_editor_text("installed".into())
        .unwrap();
    assert_eq!(host.get_editor_text().unwrap(), "installed");
    assert!(ctx.has_ui().unwrap());
    let retained = ctx.ui().unwrap();
    s.runner.set_ui_context(None);
    assert!(!ctx.has_ui().unwrap());
    assert_eq!(ctx.ui().unwrap().get_editor_text().unwrap(), "");
    assert_eq!(retained.get_editor_text().unwrap(), "installed");
}

#[test]
fn base_context_methods_forward_current_capabilities() {
    let s = served("/work");
    let recorder = Recorder::new();
    s.runner.bind_core(recorder.actions());
    let ctx = s.runner.create_context();
    assert_eq!(ctx.cwd().unwrap(), "/work");
    let history = ctx.session_manager().unwrap();
    s.history.borrow_mut().append_session_info("named").unwrap();
    assert_eq!(history.borrow().get_session_name().unwrap(), Some("named"));
    assert!(Rc::ptr_eq(&ctx.model_registry().unwrap(), &s.catalog));
    assert!(recorder.take().is_empty());
    check_bound_operations(&ctx, &recorder);
    check_compaction_forwarding(&ctx, &recorder);
    assert!(!ctx.has_ui().unwrap());
    assert_eq!(ctx.ui().unwrap().get_editor_text().unwrap(), "");
    s.runner.shutdown().unwrap();
    assert_eq!(recorder.take(), ["shutdown"]);
}

/// Each forwarding operation returns its bound value and calls only itself.
fn check_bound_operations(ctx: &ExtensionContext, recorder: &Recorder) {
    assert!(same_model(ctx.model().unwrap(), &recorder.model));
    assert_eq!(recorder.take(), ["get_model"]);
    assert!(!ctx.is_idle().unwrap());
    assert_eq!(recorder.take(), ["is_idle"]);
    assert!(ctx.signal().unwrap().is_some());
    assert_eq!(recorder.take(), ["get_signal"]);
    ctx.abort().unwrap();
    assert_eq!(recorder.take(), ["abort"]);
    assert!(ctx.has_pending_messages().unwrap());
    assert_eq!(recorder.take(), ["has_pending_messages"]);
    ctx.shutdown().unwrap();
    assert_eq!(recorder.take(), ["shutdown"]);
    let usage = ctx.get_context_usage().unwrap();
    let expected = ContextUsage {
        tokens: None,
        context_window: 200_000.0,
        percent: None,
    };
    assert_eq!(usage, Some(expected));
    assert_eq!(recorder.take(), ["get_context_usage"]);
    assert_eq!(ctx.get_system_prompt().unwrap(), "bound prompt");
    assert_eq!(recorder.take(), ["get_system_prompt"]);
}

/// Forwards absent, default and full compaction requests unchanged.
fn check_compaction_forwarding(ctx: &ExtensionContext, recorder: &Recorder) {
    let called = Rc::new(Cell::new(false));
    let full = full_compaction(&called);
    let (complete, error) = (
        full.on_complete.clone().unwrap(),
        full.on_error.clone().unwrap(),
    );
    ctx.compact(None).unwrap();
    ctx.compact(Some(CompactOptions::default())).unwrap();
    ctx.compact(Some(full)).unwrap();
    assert_eq!(recorder.take(), ["compact", "compact", "compact"]);
    let compacts = recorder.compacts.borrow();
    assert!(compacts[0].is_none());
    let default = compacts[1].as_ref().unwrap();
    assert!(default.custom_instructions.is_none());
    assert!(default.on_complete.is_none() && default.on_error.is_none());
    let forwarded = compacts[2].as_ref().unwrap();
    assert_eq!(forwarded.custom_instructions.as_deref(), Some("keep"));
    assert!(Rc::ptr_eq(
        forwarded.on_complete.as_ref().unwrap(),
        &complete
    ));
    assert!(Rc::ptr_eq(forwarded.on_error.as_ref().unwrap(), &error));
    assert!(!called.get());
}

#[test]
fn unbound_context_actions_return_runner_defaults() {
    let s = served("/work");
    let ctx = s.runner.create_context();
    assert!(ctx.model().unwrap().is_none());
    assert!(ctx.is_idle().unwrap());
    assert!(ctx.signal().unwrap().is_none());
    assert!(!ctx.has_pending_messages().unwrap());
    assert_eq!(ctx.get_context_usage().unwrap(), None);
    assert_eq!(ctx.get_system_prompt().unwrap(), "");
    let called = Rc::new(Cell::new(false));
    ctx.abort().unwrap();
    ctx.shutdown().unwrap();
    ctx.compact(None).unwrap();
    ctx.compact(Some(full_compaction(&called))).unwrap();
    s.runner.shutdown().unwrap();
    assert!(!called.get());
    assert_eq!(ctx.cwd().unwrap(), "/work");
    assert_eq!(ctx.session_manager().unwrap().borrow().get_cwd(), "/work");
    assert!(Rc::ptr_eq(&ctx.model_registry().unwrap(), &s.catalog));
}

#[test]
fn stale_access_is_catchable_without_revoking_retained_raw_objects() {
    let bus = LocalBus::new();
    let owner = loaded("/stale", bus.controller.for_extension());
    let s = served_with("/work", &owner.owners, &owner.runtime);
    let recorder = Recorder::new();
    s.runner.bind_core(recorder.actions());
    let (ui, host) = Named::install("context");
    s.runner.set_ui_context(Some(ui));
    let ctx = s.runner.create_context();
    let (ui, history) = (ctx.ui().unwrap(), ctx.session_manager().unwrap());
    let catalog = ctx.model_registry().unwrap();
    let (signal, model) = (
        ctx.signal().unwrap().unwrap(),
        ctx.model().unwrap().unwrap(),
    );
    recorder.take();
    s.runner.invalidate(None);
    assert_eq!(errors(&ctx), vec![Some(STALE.to_owned()); 14]);
    assert!(recorder.take().is_empty());
    let fresh = s.runner.create_context();
    assert_eq!(fresh.cwd().unwrap_err().to_string(), STALE);
    ui.set_editor_text("kept".into()).unwrap();
    assert_eq!(host.get_editor_text().unwrap(), "kept");
    assert_eq!(history.borrow().get_cwd(), "/work");
    assert_eq!(catalog.borrow().get_error(), None);
    signal.abort();
    assert!(signal.is_aborted());
    assert_eq!(model.read().unwrap().id, "bound");
    assert_eq!(s.runner.get_ui_context().get_editor_text().unwrap(), "kept");
    s.runner.shutdown().unwrap();
    assert_eq!(recorder.take(), ["shutdown"]);
    let command = RegisteredCommandOptions::<Local> {
        description: None,
        get_argument_completions: None,
        handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
    };
    let registered = owner.api.register_command("late".into(), command);
    assert_eq!(registered.unwrap_err().to_string(), STALE);
}

/// A boolean flag registration without a default.
fn flag() -> ExtensionFlagOptions {
    ExtensionFlagOptions {
        description: None,
        r#type: FlagType::Boolean,
        default: None,
    }
}

#[test]
fn independent_context_operations_survive_a_caught_stale_error() {
    local(async {
        let bus = LocalBus::new();
        let a_owner = loaded("/a-ext", bus.controller.for_extension());
        a_owner.api.register_flag("verbose".into(), flag()).unwrap();
        let a = served_with("/a", &a_owner.owners, &a_owner.runtime);
        let b_owner = loaded("/b-ext", bus.controller.for_extension());
        let b = served_with("/b", &b_owner.owners, &b_owner.runtime);
        let a_ctx = a.runner.create_context();
        a.runner.invalidate(None);
        let options = ExtensionShortcutOptions::<Local> {
            description: None,
            handler: Rc::new(move |ctx: ExtensionContext| {
                let a_ctx = a_ctx.clone();
                Box::pin(async move {
                    yield_now().await;
                    assert_eq!(a_ctx.cwd().unwrap_err().to_string(), STALE);
                    assert_eq!(ctx.cwd()?, "/b");
                    assert!(ctx.is_idle()?);
                    Ok(())
                })
            }),
        };
        b_owner
            .api
            .register_shortcut("ctrl+y".into(), options)
            .unwrap();
        let shortcuts = b.runner.get_shortcuts(&Vec::new());
        let handler = Rc::clone(&shortcuts["ctrl+y"].read().unwrap().handler);
        handler(b.runner.create_context()).await.unwrap();
        assert_eq!(
            a.runner.create_context().cwd().unwrap_err().to_string(),
            STALE
        );
        assert_eq!(a.runner.get_flags().keys().collect::<Vec<_>>(), ["verbose"]);
        assert_eq!(a.runner.get_extension_paths(), ["/a-ext"]);
    });
}

#[test]
fn runner_invalidation_keeps_the_first_message_including_empty() {
    let bus = LocalBus::new();
    for (messages, expected) in [(["first", "second"], "first"), (["", "later"], "")] {
        let owner = loaded("/stale", bus.controller.for_extension());
        let s = served_with("/work", &owner.owners, &owner.runtime);
        let ctx = s.runner.create_context();
        for message in messages {
            s.runner.invalidate(Some(message.into()));
            assert_eq!(ctx.cwd().unwrap_err().to_string(), expected);
            let registered = owner.api.register_flag("f".into(), flag());
            assert_eq!(registered.unwrap_err().to_string(), expected);
        }
    }
}

/// Fails with the marker naming `operation`.
fn fail<T>(operation: &'static str) -> Result<T, EventError> {
    Err(Box::new(Marker(operation)))
}

/// The marker inside an operation's error.
fn marker<T>(result: Result<T, EventError>) -> &'static str {
    result
        .err()
        .unwrap()
        .downcast_ref::<Marker<&'static str>>()
        .unwrap()
        .0
}

#[test]
fn context_actions_propagate_their_original_errors() {
    let s = served("/work");
    s.runner.bind_core(ExtensionContextActions {
        get_model: Rc::new(|| fail("get_model")),
        is_idle: Rc::new(|| fail("is_idle")),
        get_signal: Rc::new(|| fail("get_signal")),
        abort: Rc::new(|| fail("abort")),
        has_pending_messages: Rc::new(|| fail("has_pending_messages")),
        shutdown: Rc::new(|| fail("shutdown")),
        get_context_usage: Rc::new(|| fail("get_context_usage")),
        compact: Rc::new(|_| fail("compact")),
        get_system_prompt: Rc::new(|| fail("get_system_prompt")),
    });
    let ctx = s.runner.create_context();
    let markers = [
        marker(ctx.model()),
        marker(ctx.is_idle()),
        marker(ctx.signal()),
        marker(ctx.abort()),
        marker(ctx.has_pending_messages()),
        marker(ctx.shutdown()),
        marker(ctx.get_context_usage()),
        marker(ctx.compact(Some(CompactOptions::default()))),
        marker(ctx.get_system_prompt()),
        marker(s.runner.shutdown()),
    ];
    let expected = [
        "get_model",
        "is_idle",
        "get_signal",
        "abort",
        "has_pending_messages",
        "shutdown",
        "get_context_usage",
        "compact",
        "get_system_prompt",
        "shutdown",
    ];
    assert_eq!(markers, expected);
}

#[test]
fn context_exposes_the_current_cancellation_signal() {
    let s = served("/work");
    let signal = Cancellation::new();
    let bound = signal.clone();
    s.runner.bind_core(ExtensionContextActions {
        get_signal: Rc::new(move || Ok(Some(bound.clone()))),
        ..ExtensionContextActions::default()
    });
    let ctx = s.runner.create_context();
    let earlier = ctx.signal().unwrap().unwrap();
    assert!(!earlier.is_aborted());
    signal.abort();
    assert!(earlier.is_aborted());
    assert!(ctx.signal().unwrap().unwrap().is_aborted());
}

/// Requires a value that can move to another thread.
fn send<T: Send>(value: T) -> T {
    value
}

/// Requires a value that can be shared across threads.
fn send_sync<T: Send + Sync>() {}

#[test]
fn shortcut_handlers_receive_the_runner_context() {
    local(async {
        let bus = LocalBus::new();
        let owner = loaded("/keys", bus.controller.for_extension());
        let seen = Rc::new(RefCell::new(Vec::new()));
        let output = Rc::clone(&seen);
        let options = ExtensionShortcutOptions::<Local> {
            description: None,
            handler: Rc::new(move |ctx: ExtensionContext| {
                let output = Rc::clone(&output);
                Box::pin(async move {
                    yield_now().await;
                    output
                        .borrow_mut()
                        .push((ctx.cwd()?.to_owned(), ctx.has_ui()?));
                    Ok(())
                })
            }),
        };
        owner
            .api
            .register_shortcut("ctrl+k".into(), options)
            .unwrap();
        let s = served_with("/work", &owner.owners, &owner.runtime);
        let shortcuts = s.runner.get_shortcuts(&Vec::new());
        let handler = Rc::clone(&shortcuts["ctrl+k"].read().unwrap().handler);
        handler(s.runner.create_context()).await.unwrap();
        let (ui, _) = Named::install("context");
        s.runner.set_ui_context(Some(ui));
        handler(s.runner.create_context()).await.unwrap();
        let expected = [("/work".to_owned(), false), ("/work".to_owned(), true)];
        assert_eq!(*seen.borrow(), expected);
    });
    threaded_shortcut_reads_its_context_before_suspending();
}

/// A thread-safe shortcut reads its context synchronously and returns a `Send` future.
fn threaded_shortcut_reads_its_context_before_suspending() {
    send_sync::<ShortcutHandler>();
    send_sync::<Extension>();
    let owner = Arc::new(RwLock::new(Extension::<maestro_models::Threaded>::new(
        "/threaded".into(),
        "/resolved".into(),
        local_support::source("/threaded"),
    )));
    let owners = Arc::new(RwLock::new(vec![Arc::clone(&owner)]));
    let runtime = Arc::new(ExtensionRuntimeState::new());
    let bus = create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
    let api = ExtensionAPI::new(&owner, Arc::clone(&runtime), bus.for_extension());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let output = Arc::clone(&seen);
    let options: ExtensionShortcutOptions = ExtensionShortcutOptions {
        description: None,
        handler: Arc::new(move |ctx: ExtensionContext| {
            let cwd = ctx.cwd().map(str::to_owned);
            let output = Arc::clone(&output);
            Box::pin(async move {
                output.lock().unwrap().push(cwd?);
                Ok(())
            })
        }),
    };
    api.register_shortcut("ctrl+t".into(), options).unwrap();
    let runner = ExtensionRunner::<maestro_models::Threaded>::new(
        owners,
        runtime,
        runner_session("/threaded").0,
        Arc::new(|_| {}),
        LiveTheme::default(),
    );
    let shortcuts = runner.get_shortcuts(&Vec::new());
    let handler = Arc::clone(&shortcuts["ctrl+t"].read().unwrap().handler);
    let pressed = send(handler(runner.create_context()));
    let executor = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    executor.block_on(pressed).unwrap();
    assert_eq!(*seen.lock().unwrap(), ["/threaded"]);
}

#[test]
fn context_callbacks_can_rebind_without_retaining_the_registration_owner() {
    let bus = LocalBus::new();
    let owner = loaded("/context", bus.controller.for_extension());
    let s = served_with("/work", &owner.owners, &owner.runtime);
    let runner = Rc::new(s.runner);
    let weak_runner = Rc::downgrade(&runner);
    let captured = Rc::new(Cell::new(false));
    let released = Rc::downgrade(&captured);
    runner.bind_core(ExtensionContextActions {
        is_idle: Rc::new(move || {
            captured.set(true);
            weak_runner
                .upgrade()
                .unwrap()
                .bind_core(ExtensionContextActions::default());
            Ok(false)
        }),
        ..ExtensionContextActions::default()
    });
    let ctx = runner.create_context();
    assert!(!ctx.is_idle().unwrap());
    assert!(ctx.is_idle().unwrap());
    assert!(released.upgrade().is_none());
    let retained = Rc::new(Cell::new(false));
    let released = Rc::downgrade(&retained);
    runner.bind_core(ExtensionContextActions {
        is_idle: Rc::new(move || {
            retained.set(true);
            Ok(false)
        }),
        ..ExtensionContextActions::default()
    });
    owner
        .api
        .register_shortcut(
            "ctrl+k".into(),
            ExtensionShortcutOptions::<Local> {
                description: None,
                handler: Rc::new(move |_| {
                    let idle = ctx.is_idle();
                    Box::pin(async move {
                        assert!(!idle?);
                        Ok(())
                    })
                }),
            },
        )
        .unwrap();
    drop(runner);
    assert!(released.upgrade().is_some());
    drop(owner);
    assert!(released.upgrade().is_none());
}

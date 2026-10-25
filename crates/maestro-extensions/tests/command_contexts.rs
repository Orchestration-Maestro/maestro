//! Guarded command operations and owned replacement contexts.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::context_actions;
use crate::local_support;
use crate::model_support;
use crate::ui_host;
use local_support::runner;
use maestro_extensions::ExtensionContextActions;
use std::{rc::Rc, sync::RwLock};

#[test]
fn command_context_getters_remain_lazy() {
    let runner = runner(&Rc::new(RwLock::new(Vec::new())));
    let ctx = runner.create_command_context();
    runner
        .bind_core(
            <maestro_models::Local as maestro_extensions::ExtensionExecution>::unbound_actions(),
            ExtensionContextActions {
                is_idle: Rc::new(|| Ok(false)),
                get_system_prompt: Rc::new(|| Ok("B prompt".into())),
                ..ExtensionContextActions::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    let (ui, _) = ui_host::Named::install("host");
    runner.set_ui_context(Some(ui));
    assert!(!ctx.is_idle().unwrap());
    assert!(ctx.has_ui().unwrap());
    local(async {
        assert_eq!(
            ctx.ui()
                .unwrap()
                .select("pick".into(), vec!["a".into()], None)
                .await
                .unwrap()
                .as_deref(),
            Some("host:pick:a")
        );
    });
    assert!(ctx.model().unwrap().is_none());
    assert_eq!(ctx.cwd().unwrap(), "/work");
    let model = model_support::model("A");
    let captured = model.clone();
    runner
        .bind_core(
            <maestro_models::Local as maestro_extensions::ExtensionExecution>::unbound_actions(),
            ExtensionContextActions {
                get_model: Rc::new(move || Ok(Some(captured.clone()))),
                ..Default::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    let bound = runner.create_command_context();
    runner
        .bind_core(
            <maestro_models::Local as maestro_extensions::ExtensionExecution>::unbound_actions(),
            ExtensionContextActions {
                is_idle: Rc::new(|| Ok(false)),
                get_system_prompt: Rc::new(|| Ok("B prompt".into())),
                ..Default::default()
            },
            maestro_extensions::types::ProviderActions::default(),
        )
        .unwrap();
    check_late_contexts(&runner, &ctx, &bound, &model);
}

#[test]
fn command_unbinding_restores_noop_and_false_cancelled_results() {
    local_support::local(async {
        let runner = runner(&Rc::new(RwLock::new(Vec::new())));
        let ctx = runner.create_command_context();
        let record = Recorder::default();
        for bound in [false, true] {
            if bound {
                runner.bind_command_context(Some(record.actions(true)));
                runner.bind_command_context(None);
            }
            ctx.wait_for_idle().await.unwrap();
            ctx.reload().await.unwrap();
            assert!(
                full_option_commands(&ctx)
                    .await
                    .into_iter()
                    .all(|r| !r.unwrap())
            );
            assert!(!ctx.new_session(None).await.unwrap().cancelled);
            assert!(!ctx.fork("entry".into(), None).await.unwrap().cancelled);
            assert!(
                !ctx.navigate_tree("target".into(), None)
                    .await
                    .unwrap()
                    .cancelled
            );
            assert!(
                !ctx.switch_session("path".into(), None)
                    .await
                    .unwrap()
                    .cancelled
            );
            assert!(record.take().is_empty());
        }
    });
}

#[path = "support/command.rs"]
mod command;
use command::{Call, Recorder};
use local_support::local;
use maestro_extensions::{
    CommandContextOperations, ExtensionCommandContextActions, ReplacedSessionContext,
    types::{ForkOptions, NewSessionCommandOptions, SwitchSessionOptions},
};
use maestro_session::session_manager::{ForkPosition, NavigateTreeOptions, ReadonlySessionManager};
use std::cell::RefCell;

#[test]
fn command_options_preserve_absence_and_every_callback() {
    local(async {
        let runner = runner(&Rc::new(RwLock::new(Vec::new())));
        let record = Recorder::default();
        let operations = record.operations(false);
        let _: &maestro_extensions::NewSessionHandler = &operations.new_session;
        let _: &maestro_extensions::ForkHandler = &operations.fork;
        let _: &maestro_extensions::NavigateTreeHandler = &operations.navigate_tree;
        let _: &maestro_extensions::SwitchSessionHandler = &operations.switch_session;
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(operations)));
        let ctx = runner.create_command_context();
        let setup: Rc<dyn Fn(Rc<RefCell<maestro_session::SessionManager>>) -> TestFuture<()>> =
            Rc::new(|_| panic!("setup must stay with the application"));
        let with: Rc<dyn Fn(ReplacedSessionContext) -> TestFuture<()>> =
            Rc::new(|_| panic!("continuation must stay with the application"));
        for mode in 0..3 {
            send_options(&ctx, mode, &setup, &with).await;
            check_options(record.take(), mode, &setup, &with);
        }
        ctx.fork(
            "entry-1".into(),
            Some(ForkOptions {
                position: Some(ForkPosition::Before),
                with_session: None,
            }),
        )
        .await
        .unwrap();
        assert!(
            matches!(record.take().as_slice(), [Call::Fork(id, Some(options))] if id == "entry-1" && options.position == Some(ForkPosition::Before))
        );
    });
}

/// Owned local operation result used by application callbacks.
type TestFuture<T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, maestro_extensions::EventError>>>>;

#[test]
fn replacement_callbacks_receive_the_fresh_owned_context() {
    local(async {
        for operation in ["newSession", "fork", "switchSession"] {
            replacement_flow(operation).await;
        }
    });
}

use std::sync::Arc;
/// Writable-session setup callback.
type Setup = Rc<dyn Fn(Rc<RefCell<maestro_session::SessionManager>>) -> TestFuture<()>>;
/// Replacement continuation callback.
type With = Rc<dyn Fn(ReplacedSessionContext) -> TestFuture<()>>;

#[test]
fn registered_commands_receive_the_runner_command_context() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<maestro_extensions::types::CommandHandler>();
    send_sync::<maestro_extensions::RegisteredCommand>();
    send_sync::<maestro_extensions::Extension>();
    local(async {
        let bus = local_support::LocalBus::new();
        let owner = local_support::loaded("/commands", bus.controller.for_extension());
        let record = Recorder::default();
        let runner = runner(&owner.owners);
        runner.bind_command_context(Some(record.actions(false)));
        owner
            .api
            .register_command(
                "run".into(),
                maestro_extensions::types::RegisteredCommandOptions {
                    description: None,
                    get_argument_completions: None,
                    handler: Rc::new(|args, ctx| {
                        Box::pin(async move {
                            assert_eq!(args, "x y");
                            assert_eq!(ctx.cwd()?, "/work");
                            ctx.wait_for_idle().await
                        })
                    }),
                },
            )
            .unwrap();
        (runner.get_command("run").unwrap().handler)("x y".into(), runner.create_command_context())
            .await
            .unwrap();
        assert!(matches!(record.take().as_slice(), [Call::Wait]));
        bus.settle().await;
    });
    let handler: maestro_extensions::types::CommandHandler = Arc::new(|_, ctx| {
        assert_eq!(ctx.cwd().unwrap(), "/work");
        Box::pin(async { Ok(()) })
    });
    let runner = maestro_extensions::ExtensionRunner::<maestro_models::Threaded>::new(
        Arc::new(RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<maestro_models::Threaded>(),
        local_support::session::runner_session("/work").0,
        Arc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    local(async {
        handler(String::new(), runner.create_command_context())
            .await
            .unwrap();
    });
}

/// Sends absent, empty or fully populated options through every command route.
async fn send_options(
    ctx: &maestro_extensions::ExtensionCommandContext,
    mode: u8,
    setup: &Setup,
    with: &With,
) {
    let new = match mode {
        0 => None,
        1 => Some(NewSessionCommandOptions::default()),
        _ => Some(NewSessionCommandOptions {
            parent_session: Some("/sessions/parent.jsonl".into()),
            setup: Some(setup.clone()),
            with_session: Some(with.clone()),
        }),
    };
    let fork = match mode {
        0 => None,
        1 => Some(ForkOptions::default()),
        _ => Some(ForkOptions {
            position: Some(ForkPosition::At),
            with_session: Some(with.clone()),
        }),
    };
    let nav = match mode {
        0 => None,
        1 => Some(NavigateTreeOptions::default()),
        _ => Some(NavigateTreeOptions {
            summarize: Some(true),
            custom_instructions: Some("focus".into()),
            replace_instructions: Some(false),
            label: Some("checkpoint".into()),
        }),
    };
    let switch = match mode {
        0 => None,
        1 => Some(SwitchSessionOptions::default()),
        _ => Some(SwitchSessionOptions {
            with_session: Some(with.clone()),
        }),
    };
    ctx.new_session(new).await.unwrap();
    ctx.fork("entry-2".into(), fork).await.unwrap();
    ctx.navigate_tree("target".into(), nav).await.unwrap();
    ctx.switch_session("path".into(), switch).await.unwrap();
}

/// Checks absence, every field and callback identity after application delivery.
fn check_options(calls: Vec<Call>, mode: u8, setup: &Setup, with: &With) {
    let mut calls = calls.into_iter();
    let Call::New(new) = calls.next().unwrap() else {
        panic!("wrong new-session operation")
    };
    let Call::Fork(id, fork) = calls.next().unwrap() else {
        panic!("wrong fork operation")
    };
    let Call::Navigate(target, nav) = calls.next().unwrap() else {
        panic!("wrong navigation operation")
    };
    let Call::Switch(path, switch) = calls.next().unwrap() else {
        panic!("wrong switch operation")
    };
    assert_eq!(
        (id.as_str(), target.as_str(), path.as_str()),
        ("entry-2", "target", "path")
    );
    assert!(calls.next().is_none());
    if mode == 0 {
        assert!(new.is_none() && fork.is_none() && nav.is_none() && switch.is_none());
        return;
    }
    let (new, fork, nav, switch) = (new.unwrap(), fork.unwrap(), nav.unwrap(), switch.unwrap());
    if mode == 1 {
        check_empty_options(&new, &fork, &nav, &switch);
    } else {
        assert_eq!(
            new.parent_session.as_deref(),
            Some("/sessions/parent.jsonl")
        );
        assert!(Rc::ptr_eq(new.setup.as_ref().unwrap(), setup));
        for callback in [new.with_session, fork.with_session, switch.with_session] {
            assert!(Rc::ptr_eq(&callback.unwrap(), with));
        }
        assert_eq!(fork.position, Some(ForkPosition::At));
        assert_eq!(nav.summarize, Some(true));
        assert_eq!(nav.custom_instructions.as_deref(), Some("focus"));
        assert_eq!(nav.replace_instructions, Some(false));
        assert_eq!(nav.label.as_deref(), Some("checkpoint"));
    }
}

/// Drives one application replacement through its public command operation.
async fn replacement_flow(operation: &'static str) {
    let old = Rc::new(runner(&Rc::new(RwLock::new(Vec::new()))));
    let trace = Rc::new(RefCell::new(Vec::<String>::new()));
    let history = Rc::new(RefCell::new(None));
    let setup = setup_callback(trace.clone());
    let with = replacement_callback(trace.clone(), old.create_command_context());
    let weak = Rc::downgrade(&old);
    let app_trace = trace.clone();
    let app_history = history.clone();
    let apply = Rc::new(move |setup, with| {
        Box::pin(replace_session(
            weak.upgrade().unwrap(),
            app_trace.clone(),
            app_history.clone(),
            (operation, setup, with),
        )) as TestFuture<maestro_extensions::types::SessionChangeResult>
    });
    let (new, fork, switch) = (apply.clone(), apply.clone(), apply);
    old.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            new_session: Rc::new(move |options| {
                let options = options.unwrap();
                new(options.setup, options.with_session)
            }),
            fork: Rc::new(move |id, options| {
                assert_eq!(id, "entry-1");
                fork(None, options.unwrap().with_session)
            }),
            switch_session: Rc::new(move |path, options| {
                assert_eq!(path, "/s.jsonl");
                switch(None, options.unwrap().with_session)
            }),
            ..context_actions::silent_command_operations()
        },
    )));
    let ctx = old.create_command_context();
    let result = invoke_replacement(&ctx, operation, setup, with).await;
    trace
        .borrow_mut()
        .push(format!("after:cancelled={}", result.cancelled));
    assert!(ctx.cwd().is_err());
    trace.borrow_mut().push("after:old=stale".into());
    check_replacement_trace(operation, &trace, &history);
}

/// Starts the selected replacement operation with its callbacks.
async fn invoke_replacement(
    ctx: &maestro_extensions::ExtensionCommandContext,
    operation: &str,
    setup: Setup,
    with: With,
) -> maestro_extensions::types::SessionChangeResult {
    match operation {
        "newSession" => {
            ctx.new_session(Some(NewSessionCommandOptions {
                setup: Some(setup),
                with_session: Some(with),
                parent_session: None,
            }))
            .await
        }
        "fork" => {
            ctx.fork(
                "entry-1".into(),
                Some(ForkOptions {
                    with_session: Some(with),
                    ..Default::default()
                }),
            )
            .await
        }
        _ => {
            ctx.switch_session(
                "/s.jsonl".into(),
                Some(SwitchSessionOptions {
                    with_session: Some(with),
                }),
            )
            .await
        }
    }
    .unwrap()
}

/// Compares the complete application and continuation order.
fn check_replacement_trace(
    operation: &str,
    trace: &local_support::Trace,
    history: &ReplacementHistory,
) {
    let mut expected = vec![match operation {
        "fork" => "app:fork:entry-1",
        "switchSession" => "app:switchSession:/s.jsonl",
        _ => "app:newSession",
    }];
    if operation == "newSession" {
        expected.extend(["setup:cwd=/new", "app:setup-done"]);
        assert_eq!(
            history
                .borrow()
                .as_ref()
                .unwrap()
                .borrow()
                .get_entries()
                .len(),
            1
        );
    }
    expected.extend([
        "with:cwd=/new",
        "app:sent:note",
        "with:old=stale",
        "with:done",
        "after:cancelled=false",
        "after:old=stale",
    ]);
    assert_eq!(*trace.borrow(), expected);
}

/// The application-retained replacement history.
type ReplacementHistory = Rc<RefCell<Option<Rc<RefCell<maestro_session::SessionManager>>>>>;
/// Initializes a writable replacement history before its continuation.
fn setup_callback(trace: local_support::Trace) -> Setup {
    Rc::new(move |session| {
        trace
            .borrow_mut()
            .push(format!("setup:cwd={}", session.borrow().get_cwd()));
        session
            .borrow_mut()
            .append_custom_entry(
                "setup",
                maestro_session::Presence::Present(serde_json::json!({"ready":true})),
            )
            .unwrap();
        Box::pin(async { Ok(()) })
    })
}
/// Keeps the fresh context across suspension while observing the stale old one.
fn replacement_callback(
    trace: local_support::Trace,
    old: maestro_extensions::ExtensionCommandContext,
) -> With {
    Rc::new(move |ctx| {
        let (trace, old) = (trace.clone(), old.clone());
        Box::pin(async move {
            trace.borrow_mut().push(format!("with:cwd={}", ctx.cwd()?));
            ctx.wait_for_idle().await?;
            ctx.send_user_message(maestro_models::UserContent::Text("note".into()), None)
                .await?;
            assert_eq!(old.cwd().unwrap_err().to_string(), local_support::STALE);
            trace
                .borrow_mut()
                .extend(["with:old=stale".into(), "with:done".into()]);
            Ok(())
        })
    })
}
/// Simulates only the application replacement order consumed by extensions.
async fn replace_session(
    old: Rc<maestro_extensions::ExtensionRunner<maestro_models::Local>>,
    trace: local_support::Trace,
    retained: ReplacementHistory,
    options: (&str, Option<Setup>, Option<With>),
) -> Result<maestro_extensions::types::SessionChangeResult, maestro_extensions::EventError> {
    let (operation, setup, with) = options;
    trace.borrow_mut().push(
        match operation {
            "fork" => "app:fork:entry-1",
            "switchSession" => "app:switchSession:/s.jsonl",
            _ => "app:newSession",
        }
        .into(),
    );
    let (session, history) = local_support::session::runner_session("/new");
    if let Some(setup) = setup {
        setup(history.clone()).await?;
        trace.borrow_mut().push("app:setup-done".into());
    }
    *retained.borrow_mut() = Some(history);
    old.invalidate(None);
    let fresh = maestro_extensions::ExtensionRunner::<maestro_models::Local>::new(
        Rc::new(RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<maestro_models::Local>(),
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let replaced = ReplacedSessionContext::new(
        fresh.create_command_context(),
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
        Rc::new(move |content, _| {
            assert!(
                matches!(content, maestro_models::UserContent::Text(ref text) if text == "note")
            );
            trace.borrow_mut().push("app:sent:note".into());
            Box::pin(async { Ok(()) })
        }),
    );
    if let Some(with) = with {
        with(replaced).await?;
    }
    Ok(maestro_extensions::types::SessionChangeResult::default())
}

/// Checks every field of supplied empty option records.
fn check_empty_options(
    new: &NewSessionCommandOptions,
    fork: &ForkOptions,
    nav: &NavigateTreeOptions,
    switch: &SwitchSessionOptions,
) {
    assert!(new.parent_session.is_none() && new.setup.is_none() && new.with_session.is_none());
    assert!(fork.position.is_none() && fork.with_session.is_none());
    assert!(
        nav.summarize.is_none()
            && nav.custom_instructions.is_none()
            && nav.replace_instructions.is_none()
            && nav.label.is_none()
    );
    assert!(switch.with_session.is_none());
}

/// Calls all six operations in their declared order.
async fn all_commands(
    ctx: &maestro_extensions::ExtensionCommandContext,
) -> Vec<Result<bool, maestro_extensions::EventError>> {
    vec![
        ctx.wait_for_idle().await.map(|()| false),
        ctx.new_session(None).await.map(|r| r.cancelled),
        ctx.fork("  e\u{feff}\u{85} ".into(), None)
            .await
            .map(|r| r.cancelled),
        ctx.navigate_tree("  e\u{feff}\u{85} ".into(), None)
            .await
            .map(|r| r.cancelled),
        ctx.switch_session("  e\u{feff}\u{85} ".into(), None)
            .await
            .map(|r| r.cancelled),
        ctx.reload().await.map(|()| false),
    ]
}

#[test]
fn command_operations_forward_to_the_current_binding() {
    local(async {
        let runner = runner(&Rc::new(RwLock::new(Vec::new())));
        let ctx = runner.create_command_context();
        let (a, b) = (Recorder::default(), Recorder::default());
        runner.bind_command_context(Some(a.actions(true)));
        assert_eq!(
            all_commands(&ctx)
                .await
                .into_iter()
                .map(Result::unwrap)
                .collect::<Vec<_>>(),
            [false, true, true, true, true, false]
        );
        let calls = a.take();
        assert!(
            matches!(calls.as_slice(), [Call::Wait, Call::New(None), Call::Fork(id, None), Call::Navigate(target, None), Call::Switch(path, None), Call::Reload] if id == "  e\u{feff}\u{85} " && target == id && path == id)
        );
        let started = ctx.fork("before".into(), None);
        assert_eq!(a.0.borrow().len(), 1);
        runner.bind_command_context(Some(b.actions(false)));
        assert!(started.await.unwrap().cancelled);
        assert!(b.take().is_empty());
        assert_eq!(
            all_commands(&ctx)
                .await
                .into_iter()
                .map(Result::unwrap)
                .collect::<Vec<_>>(),
            [false; 6]
        );
        assert_eq!(b.take().len(), 6);
        assert_eq!(a.take().len(), 1);
    });
}

#[test]
fn stale_command_contexts_fail_before_selecting_an_operation() {
    local(async {
        let runner = runner(&Rc::new(RwLock::new(Vec::new())));
        let record = Recorder::default();
        runner.bind_command_context(Some(record.actions(true)));
        let ctx = runner.create_command_context();
        let started = ctx.new_session(None);
        runner.invalidate(None);
        assert!(started.await.unwrap().cancelled);
        assert_eq!(record.take().len(), 1);
        for result in all_commands(&ctx).await {
            assert_eq!(result.unwrap_err().to_string(), local_support::STALE);
        }
        assert!(record.take().is_empty());
        for result in full_option_commands(&ctx).await {
            assert_eq!(result.unwrap_err().to_string(), local_support::STALE);
        }
        assert!(record.take().is_empty());
        assert_stale_base(&ctx);
        assert_eq!(
            runner
                .create_command_context()
                .reload()
                .await
                .unwrap_err()
                .to_string(),
            local_support::STALE
        );
        let other = local_support::runner(&Rc::new(RwLock::new(Vec::new())));
        other.invalidate(Some("first".into()));
        other.invalidate(Some("second".into()));
        assert_eq!(
            other
                .create_command_context()
                .wait_for_idle()
                .await
                .unwrap_err()
                .to_string(),
            "first"
        );
    });
}
/// Checks every base member inherited by a command context.
fn assert_stale_base(ctx: &maestro_extensions::ExtensionCommandContext) {
    assert!(ctx.ui().is_err());
    assert!(ctx.has_ui().is_err());
    assert!(ctx.cwd().is_err());
    assert!(ctx.session_manager().is_err());
    assert!(ctx.model_registry().is_err());
    assert!(ctx.model().is_err());
    assert!(ctx.is_idle().is_err());
    assert!(ctx.signal().is_err());
    assert!(ctx.abort().is_err());
    assert!(ctx.has_pending_messages().is_err());
    assert!(ctx.shutdown().is_err());
    assert!(ctx.get_context_usage().is_err());
    assert!(ctx.compact(None).is_err());
    assert!(ctx.get_system_prompt().is_err());
}

use crate::error_support;
use error_support::Marker;
/// Returns a typed marker without reconstructing it.
fn failure<T: 'static>(index: usize) -> TestFuture<T> {
    Box::pin(async move { Err(Box::new(Marker(index)) as maestro_extensions::EventError) })
}
#[test]
fn command_operation_errors_propagate_unchanged() {
    local(async {
        let runner = runner(&Rc::new(RwLock::new(Vec::new())));
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(
            CommandContextOperations {
                wait_for_idle: Rc::new(|| failure(0)),
                new_session: Rc::new(|_| failure(1)),
                fork: Rc::new(|_, _| failure(2)),
                navigate_tree: Rc::new(|_, _| failure(3)),
                switch_session: Rc::new(|_, _| failure(4)),
                reload: Rc::new(|| failure(5)),
            },
        )));
        for (index, result) in all_commands(&runner.create_command_context())
            .await
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                result
                    .unwrap_err()
                    .downcast_ref::<Marker<usize>>()
                    .unwrap()
                    .0,
                index
            );
        }
    });
}
#[test]
fn base_and_command_bindings_are_independent() {
    local(async {
        let runner = runner(&Rc::new(RwLock::new(Vec::new())));
        let (a, c) = (Recorder::default(), Recorder::default());
        runner.bind_command_context(Some(a.actions(true)));
        let shutdowns = Rc::new(RefCell::new(0));
        let count = shutdowns.clone();
        let shutdown: maestro_extensions::ShutdownHandler = Rc::new(move || {
            *count.borrow_mut() += 1;
            Ok(())
        });
        runner
            .bind_core(
                <maestro_models::Local as maestro_extensions::ExtensionExecution>::unbound_actions(
                ),
                ExtensionContextActions {
                    shutdown,
                    is_idle: Rc::new(|| Ok(false)),
                    get_system_prompt: Rc::new(|| Ok("B".into())),
                    ..Default::default()
                },
                maestro_extensions::types::ProviderActions::default(),
            )
            .unwrap();
        let ctx = runner.create_command_context();
        assert!(ctx.new_session(None).await.unwrap().cancelled);
        assert!(!ctx.is_idle().unwrap());
        ctx.shutdown().unwrap();
        runner.bind_command_context(Some(c.actions(false)));
        assert!(!ctx.new_session(None).await.unwrap().cancelled);
        assert!(!ctx.is_idle().unwrap());
        assert_eq!(ctx.get_system_prompt().unwrap(), "B");
        ctx.shutdown().unwrap();
        assert_eq!(*shutdowns.borrow(), 2);
        assert_eq!(a.take().len(), 1);
        assert_eq!(c.take().len(), 1);
    });
}
#[test]
fn reload_keeps_the_old_command_frame_running_with_a_stale_context() {
    local(async {
        let bus = local_support::LocalBus::new();
        let owner = local_support::loaded("/reload", bus.controller.for_extension());
        let runner = Rc::new(runner(&owner.owners));
        let weak = Rc::downgrade(&runner);
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(
            CommandContextOperations {
                reload: Rc::new(move || {
                    weak.upgrade().unwrap().invalidate(None);
                    Box::pin(async { Ok(()) })
                }),
                ..context_actions::silent_command_operations()
            },
        )));
        owner
            .api
            .register_command(
                "reload".into(),
                maestro_extensions::types::RegisteredCommandOptions {
                    description: None,
                    get_argument_completions: None,
                    handler: Rc::new(|_, ctx| {
                        Box::pin(async move {
                            let history = ctx.session_manager()?;
                            ctx.reload().await?;
                            assert_eq!(ctx.cwd().unwrap_err().to_string(), local_support::STALE);
                            assert_eq!(history.borrow().get_cwd(), "/work");
                            Ok(())
                        })
                    }),
                },
            )
            .unwrap();
        (runner.get_command("reload").unwrap().handler)(
            String::new(),
            runner.create_command_context(),
        )
        .await
        .unwrap();
        bus.settle().await;
    });
}

#[test]
fn replaced_context_forwards_both_message_operations() {
    local(async {
        let runner = runner(&Rc::new(RwLock::new(Vec::new())));
        let record = Recorder::default();
        runner.bind_command_context(Some(record.actions(true)));
        let messages = Rc::new(RefCell::new(Vec::new()));
        let users = messages.clone();
        let customs = messages.clone();
        let ctx = ReplacedSessionContext::new(
            runner.create_command_context(),
            Rc::new(move |message, options| {
                if let maestro_session::Presence::Present(details) = &message.details {
                    assert_ne!(details.as_f64().map(f64::to_bits), Some(0.0_f64.to_bits()));
                    assert!(
                        details
                            .as_object()
                            .is_none_or(|object| !object.contains_key("z")
                                || object.keys().map(String::as_str).eq(["z", "a"]))
                    );
                }
                customs
                    .borrow_mut()
                    .push(serde_json::json!([message, options]));
                Box::pin(async { Ok(()) })
            }),
            Rc::new(move |content, options| {
                users
                    .borrow_mut()
                    .push(serde_json::json!([content, options]));
                Box::pin(async { Ok(()) })
            }),
        );
        assert_eq!(ctx.cwd().unwrap(), "/work");
        assert!(
            ctx.switch_session("next".into(), None)
                .await
                .unwrap()
                .cancelled
        );
        assert!(matches!(record.take().as_slice(), [Call::Switch(path, None)] if path == "next"));
        message_roundtrip(&ctx, &messages).await;
        runner.invalidate(None);
        assert!(ctx.cwd().is_err());
        assert!(ctx.wait_for_idle().await.is_err());
        message_roundtrip(&ctx, &messages).await;
        message_errors(&runner).await;
    });
}
/// Exercises message absence, null, full options and both user-content forms.
async fn message_roundtrip(
    ctx: &ReplacedSessionContext,
    messages: &Rc<RefCell<Vec<serde_json::Value>>>,
) {
    use maestro_session::messages::{MessageDelivery, SendMessageOptions};
    use serde_json::json;
    for details in [None, Some(serde_json::Value::Null), Some(json!({"k":1}))] {
        let modes = if details.is_none() { 0..6 } else { 0..3 };
        for mode in modes {
            let mut input = json!({"customType":"note","content":"hello","display":true});
            if let Some(details) = &details {
                input["details"] = details.clone();
            }
            let options = match mode {
                0 => None,
                1 => Some(SendMessageOptions::default()),
                2 => Some(SendMessageOptions {
                    trigger_turn: Some(true),
                    deliver_as: Some(MessageDelivery::NextTurn),
                }),
                3 => Some(SendMessageOptions {
                    trigger_turn: Some(false),
                    deliver_as: None,
                }),
                4 => Some(SendMessageOptions {
                    trigger_turn: None,
                    deliver_as: Some(MessageDelivery::Steer),
                }),
                _ => Some(SendMessageOptions {
                    trigger_turn: None,
                    deliver_as: Some(MessageDelivery::FollowUp),
                }),
            };
            ctx.send_message(serde_json::from_value(input.clone()).unwrap(), options)
                .await
                .unwrap();
            let expected_options = match mode {
                0 => serde_json::Value::Null,
                1 => json!({}),
                2 => json!({"triggerTurn":true,"deliverAs":"nextTurn"}),
                3 => json!({"triggerTurn":false}),
                4 => json!({"deliverAs":"steer"}),
                _ => json!({"deliverAs":"followUp"}),
            };
            assert_eq!(
                std::mem::take(&mut *messages.borrow_mut()),
                [json!([input, expected_options])]
            );
        }
    }
    message_field_roundtrip(ctx, messages).await;
    user_message_roundtrip(ctx, messages).await;
}

/// Exercises one independent custom-message field at a time.
async fn message_field_roundtrip(
    ctx: &ReplacedSessionContext,
    messages: &Rc<RefCell<Vec<serde_json::Value>>>,
) {
    use serde_json::json;
    for (field, value) in [
        ("details", json!("details")),
        ("details", json!(13.25)),
        ("details", json!(false)),
        ("details", json!([null, true])),
        ("details", json!(-0.0)),
        ("details", json!({"z":1,"a":2})),
        (
            "content",
            json!([{"type":"text","text":"a"},{"type":"image","data":"image-data","mimeType":"image/png"}]),
        ),
        ("content", json!("")),
        ("content", json!([])),
        ("display", json!(false)),
    ] {
        let mut input = json!({"customType":"note","content":"hello","display":true});
        input[field] = value;
        ctx.send_message(serde_json::from_value(input.clone()).unwrap(), None)
            .await
            .unwrap();
        assert_eq!(
            std::mem::take(&mut *messages.borrow_mut()),
            [json!([input, null])]
        );
    }
}

/// Message operations preserve typed failures even when stale.
async fn message_errors(runner: &maestro_extensions::ExtensionRunner<maestro_models::Local>) {
    let errors = ReplacedSessionContext::new(
        runner.create_command_context(),
        Rc::new(|_, _| failure(10)),
        Rc::new(|_, _| failure(11)),
    );
    let custom = serde_json::from_value(
        serde_json::json!({"customType":"note","content":"hello","display":true}),
    )
    .unwrap();
    assert_eq!(
        errors
            .send_message(custom, None)
            .await
            .unwrap_err()
            .downcast_ref::<Marker<usize>>()
            .unwrap()
            .0,
        10
    );
    assert_eq!(
        errors
            .send_user_message(maestro_models::UserContent::Text("hello".into()), None)
            .await
            .unwrap_err()
            .downcast_ref::<Marker<usize>>()
            .unwrap()
            .0,
        11
    );
}

/// User content and delivery options reach the creating session unchanged.
async fn user_message_roundtrip(
    ctx: &ReplacedSessionContext,
    messages: &Rc<RefCell<Vec<serde_json::Value>>>,
) {
    use maestro_session::messages::{SendUserMessageOptions, UserMessageDelivery};
    use serde_json::json;
    for content in [
        json!("hello"),
        json!([{"type":"text","text":"a"}, {"type":"image","data":"image-data","mimeType":"image/png"}]),
        json!(""),
        json!([]),
    ] {
        let modes = if content == json!("") || content == json!([]) {
            0..1
        } else {
            0..4
        };
        for mode in modes {
            let options = match mode {
                0 => None,
                1 => Some(SendUserMessageOptions {
                    deliver_as: Some(UserMessageDelivery::Steer),
                }),
                2 => Some(SendUserMessageOptions {
                    deliver_as: Some(UserMessageDelivery::FollowUp),
                }),
                _ => Some(SendUserMessageOptions::default()),
            };
            ctx.send_user_message(serde_json::from_value(content.clone()).unwrap(), options)
                .await
                .unwrap();
            let expected_options = match mode {
                0 => serde_json::Value::Null,
                1 => json!({"deliverAs":"steer"}),
                2 => json!({"deliverAs":"followUp"}),
                _ => json!({}),
            };
            assert_eq!(
                std::mem::take(&mut *messages.borrow_mut()),
                [json!([content, expected_options])]
            );
        }
    }
}

/// Full option records whose callbacks must never run in default or stale paths.
async fn full_option_commands(
    ctx: &maestro_extensions::ExtensionCommandContext,
) -> Vec<Result<bool, maestro_extensions::EventError>> {
    let setup: Setup = Rc::new(|_| panic!("setup unexpectedly invoked"));
    let with: With = Rc::new(|_| panic!("continuation unexpectedly invoked"));
    vec![
        ctx.new_session(Some(NewSessionCommandOptions {
            parent_session: Some("/sessions/parent.jsonl".into()),
            setup: Some(setup),
            with_session: Some(with.clone()),
        }))
        .await
        .map(|r| r.cancelled),
        ctx.fork(
            "entry-1".into(),
            Some(ForkOptions {
                position: Some(ForkPosition::Before),
                with_session: Some(with.clone()),
            }),
        )
        .await
        .map(|r| r.cancelled),
        ctx.navigate_tree(
            "target".into(),
            Some(NavigateTreeOptions {
                summarize: Some(true),
                custom_instructions: Some("focus".into()),
                replace_instructions: Some(false),
                label: Some("checkpoint".into()),
            }),
        )
        .await
        .map(|r| r.cancelled),
        ctx.switch_session(
            "path".into(),
            Some(SwitchSessionOptions {
                with_session: Some(with),
            }),
        )
        .await
        .map(|r| r.cancelled),
    ]
}

fn check_late_contexts(
    runner: &maestro_extensions::ExtensionRunner<maestro_models::Local>,
    ctx: &maestro_extensions::ExtensionCommandContext,
    bound: &maestro_extensions::ExtensionCommandContext,
    model: &Arc<RwLock<maestro_models::Model>>,
) {
    assert!(Arc::ptr_eq(&bound.model().unwrap().unwrap(), model));
    assert!(!bound.is_idle().unwrap());
    assert_eq!(bound.get_system_prompt().unwrap(), "B prompt");
    runner.set_ui_context(None);
    assert!(!ctx.has_ui().unwrap());
    local(async {
        assert!(
            ctx.ui()
                .unwrap()
                .select("fallback".into(), vec![], None)
                .await
                .unwrap()
                .is_none()
        );
    });
}

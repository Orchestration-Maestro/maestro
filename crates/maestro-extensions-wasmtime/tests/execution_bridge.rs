//! Component callbacks invoked by the real registration loader and runner.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, SessionStartEvent, SessionStartReason, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
};
use maestro_extensions_wasmtime::artifact_factory;
use maestro_session::Presence;
use std::{cell::RefCell, rc::Rc, sync::RwLock};

use super::{session, support};
use support::bind_prompt;

#[test]
fn real_contexts_reach_component_commands_and_hooks() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, composed_contexts());
}

/// Exercises actual callback registration, context selection and attributed hook errors.
async fn composed_contexts() {
    let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
    let runtime = create_extension_runtime::<Local>();
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("this fixture registers no event tails")),
        Rc::new(|_| panic!("this fixture registers no bus listeners")),
    );
    let owner = load_extension_from_factory(
        artifact_factory(path.clone(), support::credential_operations()),
        "/factory-only".into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path.clone()),
    )
    .await
    .unwrap();
    {
        let registered = owner.read().unwrap();
        assert_eq!(
            registered.commands.keys().collect::<Vec<_>>(),
            ["bridge-observe"]
        );
        assert_eq!(
            registered.handlers.keys().collect::<Vec<_>>(),
            ["session_start"]
        );
        assert_eq!(registered.path, path);
        assert!(std::sync::Arc::ptr_eq(
            &registered.commands["bridge-observe"]
                .read()
                .unwrap()
                .source_info,
            &registered.source_info,
        ));
    }
    let owners = Rc::new(RwLock::new(vec![owner]));
    let runner = ExtensionRunner::<Local>::new(
        owners.clone(),
        runtime.clone(),
        session::runner_session("/command-domain").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    bind_prompt(&runner, "command prompt");
    let hook_runner = ExtensionRunner::<Local>::new(
        owners,
        runtime,
        session::runner_session("/hook-domain").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    bind_prompt(&hook_runner, "hook prompt");
    observe_callbacks(&runner, &hook_runner).await;
}

/// Rejects loading-time defaults and a command context accidentally reused for the hook.
async fn observe_callbacks(
    command_runner: &ExtensionRunner<Local>,
    hook_runner: &ExtensionRunner<Local>,
) {
    let command = command_runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let observed = command(
        "literal args".into(),
        command_runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert_eq!(
        observed,
        "command:literal args|/command-domain|command prompt"
    );
    let control = command("literal args".into(), hook_runner.create_command_context())
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(control, "command:literal args|/hook-domain|hook prompt");
    assert_ne!(observed, control);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let received = errors.clone();
    let _subscription = hook_runner.on_error(Rc::new(move |error| {
        received.borrow_mut().push((
            error.extension_path.clone(),
            error.event.clone(),
            error.error.clone(),
        ));
        Ok(())
    }));
    let mut event = SessionStartEvent {
        reason: SessionStartReason::Startup,
        previous_session_file: Presence::Missing,
    };
    assert!(hook_runner.emit(&mut event).await.unwrap().is_none());
    let errors = errors.borrow();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].0,
        std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap()
    );
    assert_eq!(errors[0].1, "session_start");
    assert_eq!(errors[0].2, "hook:/hook-domain|hook prompt");
}

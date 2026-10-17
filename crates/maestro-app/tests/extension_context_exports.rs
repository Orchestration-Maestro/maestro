//! Base extension context records through the application root.
use maestro_app::{CompactOptions, ContextUsage, ExtensionContext, ExtensionContextActions};
use maestro_catalog::ModelRegistry;
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_extensions::ExtensionRunner;
use maestro_extensions::runner::ExtensionRunnerSession;
use maestro_extensions::types::ExtensionRuntimeState;
use maestro_session::SessionManager;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The default value of an operand whose type the application does not name,
/// such as the runner's live theme.
fn default_operand<T: Default>() -> T {
    T::default()
}

#[test]
fn native_context_exports_share_the_extension_records() {
    let usage = ContextUsage {
        tokens: Some(1_000.0),
        context_window: 200_000.0,
        percent: Some(0.5),
    };
    let requests = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&requests);
    let actions = ExtensionContextActions {
        get_context_usage: Rc::new(move || Ok(Some(usage))),
        compact: Rc::new(move |options: Option<CompactOptions>| {
            recorded
                .borrow_mut()
                .push(options.and_then(|o| o.custom_instructions));
            Ok(())
        }),
        ..ExtensionContextActions::default()
    };
    let runner = runner();
    runner.bind_core(actions);
    let ctx: ExtensionContext = runner.create_context();
    assert_eq!(ctx.get_context_usage().unwrap(), Some(usage));
    let request = CompactOptions {
        custom_instructions: Some("keep".into()),
        ..CompactOptions::default()
    };
    ctx.compact(Some(request)).unwrap();
    assert_eq!(*requests.borrow(), [Some("keep".to_owned())]);
}

#[test]
fn native_command_context_exports_share_the_extension_records() {
    let runner = runner();
    let actions = maestro_app::ExtensionCommandContextActions {
        new_session: Rc::new(|_| {
            Box::pin(async {
                Ok(maestro_extensions::types::SessionChangeResult { cancelled: true })
            })
        }),
        ..Default::default()
    };
    runner.bind_command_context(Some(actions));
    let ctx: maestro_app::ExtensionCommandContext = runner.create_command_context();
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            assert!(ctx.new_session(None).await.unwrap().cancelled);
        });
}

/// Builds the real in-memory runner shared by application context export witnesses.
fn runner() -> ExtensionRunner {
    let auth = Arc::new(AuthStorage::in_memory(AuthStorageData::default()));
    let session = ExtensionRunnerSession {
        cwd: "/work".into(),
        session_manager: Rc::new(RefCell::new(SessionManager::in_memory(None).unwrap())),
        model_registry: Rc::new(RefCell::new(ModelRegistry::in_memory(auth).unwrap())),
    };
    ExtensionRunner::<maestro_models::Threaded>::new(
        Arc::new(RwLock::new(Vec::new())),
        Arc::new(ExtensionRuntimeState::new()),
        session,
        Arc::new(|_| {}),
        default_operand(),
    )
}

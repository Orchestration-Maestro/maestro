//! Compiled author setup through the production component factory.
use maestro_extensions::{ExtensionRunner, Local, create_extension_runtime};
use std::{rc::Rc, sync::RwLock};

/// Loads the shared author artifact with the real native registration and context owners.
pub(super) async fn loaded() -> ExtensionRunner<Local> {
    let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
    let runtime = create_extension_runtime::<Local>();
    let events = maestro_extensions::create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|_| {}));
    let owner = maestro_extensions::load_extension_from_factory(
        crate::artifact_factory(path.clone(), "builtin-test-version".into()),
        "/".into(),
        events.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/builtin-caller").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}

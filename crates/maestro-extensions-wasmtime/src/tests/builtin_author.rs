//! Compiled author setup through the production component factory.
use maestro_extensions::{ExtensionRunner, Local, create_extension_runtime};
use std::{rc::Rc, sync::RwLock};

/// Loads the shared author artifact with the real native registration and context owners.
pub(super) async fn loaded() -> ExtensionRunner<Local> {
    loaded_with_events().await.0
}

/// Keeps the real native event bus available for observing authored effects.
pub(super) async fn loaded_with_events() -> (
    ExtensionRunner<Local>,
    maestro_extensions::EventBusController<Local>,
) {
    loaded_with_tool_services(maestro_extensions::ToolConstruction {
        tools_manager: std::sync::Arc::new(
            maestro_extensions::ToolsManager::new("/unused-builtin-bin".into(), None)
                .with_offline(Some(true)),
        ),
        bin_dir: "/unused-builtin-bin".into(),
    })
    .await
}

/// Supplies the same application-owned tool services before the compiled factory runs.
pub(super) async fn loaded_with_tool_services(
    services: maestro_extensions::ToolConstruction,
) -> (
    ExtensionRunner<Local>,
    maestro_extensions::EventBusController<Local>,
) {
    loaded_with_reporter(services, Rc::new(|_| {})).await
}

/// Observes callback diagnostics through the original event bus reporter.
pub(super) async fn loaded_with_reporter(
    services: maestro_extensions::ToolConstruction,
    report: maestro_extensions::ErrorReporter<Local>,
) -> (
    ExtensionRunner<Local>,
    maestro_extensions::EventBusController<Local>,
) {
    let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
    let runtime = create_extension_runtime::<Local>();
    runtime.set_tool_construction(services);
    let events = maestro_extensions::create_event_bus_for::<Local>(Rc::new(drop), report);
    let owner = maestro_extensions::load_extension_from_factory(
        crate::artifact_factory(
            path.clone(),
            crate::tests::support::widgets(),
            crate::Installation {
                version: "builtin-test-version".into(),
                agent_dir: "/controlled-agent".into(),
            },
        ),
        "/".into(),
        events.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/builtin-caller").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    (runner, events)
}

/// Runs a real-search witness with pinned executables, never the runner's search PATH.
pub(super) fn with_search_tools(test: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return true;
    }
    let bin = maestro_test_directory::Scratch::new().unwrap();
    for (name, variable) in [("rg", "MAESTRO_TEST_RG"), ("fd", "MAESTRO_TEST_FD")] {
        let source = std::env::var_os(variable)
            .unwrap_or_else(|| panic!("{name}: nextest must supply {variable}"));
        let target = bin
            .path()
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(&source, target)
            .unwrap_or_else(|error| panic!("{name}: cannot copy pinned executable: {error}"));
    }
    let output = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("search-tools")
        .nocapture()
        .env("PATH", bin.path())
        .env("MAESTRO_TEST_SEARCH_BIN", bin.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

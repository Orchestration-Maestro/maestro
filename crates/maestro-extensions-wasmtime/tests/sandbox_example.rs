//! The shipped sandbox through its compiled component and native bash factory.
#![cfg(test)]

use maestro_extensions::{
    ExtensionRunner, Local, create_extension_runtime, load_extension_from_factory,
};
use maestro_extensions_wasmtime::{Installation, artifact_factory};
use std::{path::Path, rc::Rc, sync::RwLock};

/// Loads the actual component with a disposable configuration directory.
async fn loaded(root: &Path) -> Rc<ExtensionRunner<Local>> {
    let path = std::env::var("MAESTRO_SANDBOX_COMPONENT").unwrap();
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|tail| {
            drop(tokio::task::spawn_local(tail));
        }),
        Rc::new(|error| panic!("unexpected sandbox tail error: {error:?}")),
    );
    let runtime = create_extension_runtime::<Local>();
    let bin_dir = root.to_str().unwrap().to_owned();
    runtime.set_tool_construction(maestro_extensions::ToolConstruction {
        tools_manager: std::sync::Arc::new(
            maestro_extensions::ToolsManager::new(bin_dir.clone(), None).with_offline(Some(true)),
        ),
        bin_dir,
    });
    let owner = load_extension_from_factory(
        artifact_factory(
            path.clone(),
            super::support::widgets(),
            Installation {
                version: "controlled-host".into(),
                agent_dir: root.to_str().unwrap().into(),
            },
        ),
        root.to_str().unwrap().into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(root.to_str().unwrap()).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ))
}

#[cfg(target_os = "linux")]
mod linux;

#[test]
fn sandbox_registers_opt_in_controls_and_bash_override() {
    super::wasi_support::run(async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let runner = loaded(scratch.path()).await;
        assert!(runner.get_flags().contains_key("no-sandbox"));
        assert!(runner.get_command("sandbox").is_some());
        let tools = runner.get_all_registered_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].metadata().definition.name, "bash");
        assert_eq!(tools[0].metadata().label, "bash (sandboxed)");
    });
}

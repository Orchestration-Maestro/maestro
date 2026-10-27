//! The shipped bash hook through its compiled component and the native tool factory.
#![cfg(test)]
#![cfg(unix)]

use maestro_extensions::{
    ExtensionRunner, Local, create_extension_runtime, load_extension_from_factory,
};
use serde_json::json;
use std::{path::Path, rc::Rc, sync::RwLock};

/// Loads the actual component while giving execution contexts a different directory.
async fn loaded(root: &Path) -> Rc<ExtensionRunner<Local>> {
    let path = std::env::var("MAESTRO_BASH_SPAWN_HOOK_COMPONENT").unwrap();
    let bus = super::components::TracedBus::new();
    let runtime = create_extension_runtime::<Local>();
    let bin_dir = root.to_str().unwrap().to_owned();
    runtime.set_tool_construction(maestro_extensions::ToolConstruction {
        tools_manager: std::sync::Arc::new(
            maestro_extensions::ToolsManager::new(bin_dir.clone(), None).with_offline(Some(true)),
        ),
        bin_dir,
    });
    let context_cwd = root.join("session").to_str().unwrap().to_owned();
    let owner = load_extension_from_factory(
        crate::artifact(path.clone()),
        context_cwd.clone(),
        bus.controller.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(&context_cwd).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ))
}

/// Sources an isolated profile before the command, preserves cwd/environment and replaces the marker.
#[test]
fn bash_spawn_hook_sources_profile_and_preserves_native_execution() {
    if std::env::var_os("MAESTRO_BASH_HOOK_CHILD").is_none() {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        std::fs::create_dir(scratch.path().join("session")).unwrap();
        std::fs::write(
            scratch.path().join(".profile"),
            "profile_value() { printf 'profile-first\\n'; }\n",
        )
        .unwrap();
        let component = std::env::var("MAESTRO_BASH_SPAWN_HOOK_COMPONENT").unwrap();
        let output = maestro_test_process::TestProcess::current(
            "bash_spawn_hook_sources_profile_and_preserves_native_execution",
        )
        .unwrap()
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", scratch.path())
        .env("MAESTRO_BASH_HOOK_CHILD", "1")
        .env("MAESTRO_BASH_SPAWN_HOOK_COMPONENT", component)
        .env("MAESTRO_SPAWN_HOOK", "old-value")
        .env("KEPT", "inherited-value")
        .current_dir(scratch.path())
        .output()
        .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    super::wasi_support::run(async {
        let root = std::env::current_dir().unwrap();
        let runner = loaded(&root).await;
        let registered = runner.get_all_registered_tools();
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0].metadata().definition.name, "bash");
        let tool = maestro_extensions::wrap_registered_tool(&registered[0], runner);
        for _ in 0..2 {
            let result = (tool.execute)(
                "controlled-bash".into(),
                json!({"command":"profile_value; printf '%s\\n' \"$MAESTRO_SPAWN_HOOK\" \"$KEPT\"; pwd -P", "timeout":10}),
                None,
                None,
            )
            .await
            .unwrap();
            assert_eq!(
                serde_json::to_value(result.content).unwrap(),
                json!([{"type":"text","text":format!("profile-first\n1\ninherited-value\n{}\n", root.display())}])
            );
        }
    });
}

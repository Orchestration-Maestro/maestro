//! Shipped SSH operations through the component runner and original coding tools.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, create_extension_runtime, load_extension_from_factory,
};
use serde_json::{Value, json};
use std::{path::Path, rc::Rc, sync::RwLock};

/// Loads the shipped example without enabling remote execution.
async fn loaded(root: &Path) -> Rc<ExtensionRunner<Local>> {
    let path = std::env::var("MAESTRO_SSH_COMPONENT").unwrap();
    let runtime = create_extension_runtime::<Local>();
    let bus = super::components::TracedBus::new();
    let cwd = root.to_str().unwrap();
    let owner = load_extension_from_factory(
        crate::artifact(path.clone()),
        cwd.into(),
        bus.controller.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(cwd).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ))
}

/// Invokes the registered override through the application's ordinary wrapper.
async fn call(runner: &Rc<ExtensionRunner<Local>>, name: &str, params: Value) -> Value {
    let tools = runner.get_all_registered_tools();
    let registered = tools
        .iter()
        .find(|tool| tool.metadata().definition.name == name)
        .unwrap();
    let tool = maestro_extensions::wrap_registered_tool(registered, runner.clone());
    let result = (tool.execute)("ssh-call".into(), params, None, None)
        .await
        .unwrap();
    serde_json::to_value(result.content).unwrap()
}

#[test]
fn ssh_unset_keeps_original_local_coding_tools() {
    super::wasi_support::run(async {
        let root = maestro_test_directory::Scratch::new().unwrap();
        let runner = loaded(root.path()).await;
        assert_eq!(runner.get_all_registered_tools().len(), 4);
        assert!(runner.get_flags().contains_key("ssh"));
        call(
            &runner,
            "write",
            json!({"path":"note.txt","content":"before\n"}),
        )
        .await;
        assert_eq!(
            call(&runner, "read", json!({"path":"note.txt"})).await[0]["text"],
            "before\n"
        );
        call(
            &runner,
            "edit",
            json!({"path":"note.txt","edits":[{"oldText":"before","newText":"after"}]}),
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(root.path().join("note.txt")).unwrap(),
            "after\n"
        );
        assert_eq!(
            call(&runner, "bash", json!({"command":"printf local"})).await[0]["text"],
            "local"
        );
    });
}

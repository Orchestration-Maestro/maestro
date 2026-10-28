//! Packaged component tools through real SDK session loading and reload.
use crate::support::{Root, session::component_session};
use maestro_app::{AgentSession, PromptOptions};
use serde_json::{Value, json};
use std::rc::Rc;

/// Execute the actual installed tool callback, outside every session and tool guard.
async fn execute(session: &AgentSession, name: &str, args: Value) -> Value {
    let execute = {
        let state = session.state().read().unwrap();
        let tools = state.tools().read().unwrap();
        let tool = tools
            .iter()
            .find(|tool| tool.read().unwrap().definition.name == name)
            .unwrap();
        tool.read().unwrap().execute.clone()
    };
    serde_json::to_value(execute("sdk-call".into(), args, None, None).await.unwrap()).unwrap()
}

/// Reload reads replaced package bytes and removes the old executable tool.
#[test]
fn sdk_loads_and_reloads_packaged_component_tools() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = Root::new();
        let package = crate::support::resources(&root);
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"sdk-component","maestro":{"extensions":["index.wasm"]}}"#,
        )
        .unwrap();
        let session = component_session(
            &root,
            local.clone(),
            package.display().to_string(),
            None,
            crate::support::widgets(),
        )
        .await;
        let before = session.extension_runner().create_context();
        assert!(before.cwd().unwrap().ends_with("project"));
        assert_eq!(
            execute(&session, "resource-one", json!({})).await["content"][0]["text"],
            "one"
        );
        std::fs::copy(
            std::env::var_os("MAESTRO_RESOURCE_RELOADED_COMPONENT").unwrap(),
            package.join("index.wasm"),
        )
        .unwrap();
        session
            .prompt("/observe-resources reload", PromptOptions::default())
            .await
            .unwrap();
        assert!(
            !session
                .get_active_tool_names()
                .iter()
                .any(|name| name == "resource-one")
        );
        assert_eq!(
            execute(&session, "resource-two", json!({})).await["content"][0]["text"],
            "two"
        );
        assert_eq!(
            before.cwd().unwrap(),
            session.extension_runner().create_context().cwd().unwrap()
        );
        session.dispose();
    });
}

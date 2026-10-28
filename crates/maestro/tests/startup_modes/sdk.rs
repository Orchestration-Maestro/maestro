//! Packaged component tools through real SDK session loading and reload.
use crate::support::{Root, session::component_session};
use maestro_app::AgentSession;
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
        session.reload().await.unwrap();
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

/// Both standard generations operate on the same native bytes through an actual SDK tool.
#[test]
fn sdk_component_uses_async_filesystem_and_standard_library_bytes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = Root::new();
        let session = component_session(
            &root,
            local.clone(),
            std::env::var("MAESTRO_SDK_STANDARD_COMPONENT").unwrap(),
            None,
            crate::support::widgets(),
        )
        .await;
        let path = root.path("project/shared.bin");
        for (generation, bytes) in [("p3", vec![0, 255, 7]), ("p2", vec![1, 254, 8, 0])] {
            let written = execute(
                &session,
                "sdk-standard",
                json!({"generation":generation,"path":path,"write":true,"bytes":bytes}),
            )
            .await;
            assert_eq!(written["details"]["bytes"], json!(bytes));
            assert_eq!(
                written["details"]["cwd"],
                root.path("project").display().to_string()
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            let other = if generation == "p3" { "p2" } else { "p3" };
            let read = execute(
                &session,
                "sdk-standard",
                json!({"generation":other,"path":path,"write":false,"bytes":[]}),
            )
            .await;
            assert_eq!(read["details"]["bytes"], json!(bytes));
        }
        session.dispose();
    });
}

/// A dependency-backed author workload executes after package discovery, not a test-host import.
#[test]
fn sdk_executes_a_packaged_dependency_backed_author_workload() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = Root::new();
        let package = root.path("project/duration-package");
        std::fs::create_dir(&package).unwrap();
        std::fs::copy(std::env::var_os("MAESTRO_WITH_DEPS_COMPONENT").unwrap(), package.join("duration.wasm")).unwrap();
        std::fs::write(package.join("package.json"), r#"{"name":"duration-package","maestro":{"extensions":["duration.wasm"]}}"#).unwrap();
        let session = component_session(&root, local.clone(), package.display().to_string(), None, crate::support::widgets()).await;
        assert_eq!(execute(&session, "parse_duration", json!({"duration":"2 days"})).await["content"][0]["text"], "2 days = 172800000 milliseconds");
        assert_eq!(execute(&session, "parse_duration", json!({"duration":"1h"})).await["content"][0]["text"], "1h = 3600000 milliseconds");
        session.dispose();
    });
}

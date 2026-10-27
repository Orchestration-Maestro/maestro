//! Minimal displays through the shipped component and original native tool owners.
#![cfg(test)]
use maestro_extensions::{ExtensionRunner, Local};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{path::Path, rc::Rc, sync::RwLock};

/// Loads the one shipped example with native factories and a disposable session cwd.
async fn loaded(cwd: &Path) -> Rc<ExtensionRunner<Local>> {
    let path = super::extension_guide::cargo::component(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-extensions-wasm"
        )),
        &["--example", "minimal_mode"],
        "minimal_mode",
    )
    .unwrap();
    let path = path.to_str().unwrap().to_owned();
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no background work")),
        Rc::new(|error| panic!("{error}")),
    );
    let owner = maestro_extensions::load_extension_from_factory(
        crate::artifact(path.clone()),
        cwd.to_str().unwrap().into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(cwd.to_str().unwrap()).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ))
}

/// Runs a component on its caller-owned local executor.
fn run(action: impl std::future::Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}

/// Invokes a registered override, not a replacement implementation in the test.
async fn execute(runner: &Rc<ExtensionRunner<Local>>, name: &str, args: Value) -> Value {
    let registered = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap();
    let tool = maestro_extensions::wrap_registered_tool(&registered, runner.clone());
    let result = (tool.execute)("minimal".into(), args, None, None)
        .await
        .unwrap();
    serde_json::to_value(result).unwrap()
}

#[test]
fn minimal_mode_retains_builtin_execution_in_the_session_directory() {
    run(async {
        let root = Scratch::new().unwrap();
        std::fs::write(root.path().join("input.txt"), "one\ntwo\nthree").unwrap();
        let runner = loaded(root.path()).await;
        let names: Vec<_> = runner
            .get_all_registered_tools()
            .iter()
            .map(|tool| tool.definition.name())
            .collect();
        assert_eq!(
            names,
            ["read", "bash", "write", "edit", "find", "grep", "ls"]
        );
        for _ in 0..2 {
            let result = execute(
                &runner,
                "read",
                json!({"path":"input.txt","offset":2,"limit":1}),
            )
            .await;
            assert_eq!(
                result["content"][0]["text"],
                "two\n\n[1 more lines in file. Use offset=3 to continue.]"
            );
        }
        execute(
            &runner,
            "write",
            json!({"path":"nested/out.txt","content":"before"}),
        )
        .await;
        execute(
            &runner,
            "edit",
            json!({"path":"nested/out.txt","edits":[{"oldText":"before","newText":"after"}]}),
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(root.path().join("nested/out.txt")).unwrap(),
            "after"
        );
        let result = execute(&runner, "bash", json!({"command":"printf controlled"})).await;
        assert_eq!(result["content"][0]["text"], "controlled");
        let result = execute(&runner, "ls", json!({"path":"nested"})).await;
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("out.txt")
        );
    });
}

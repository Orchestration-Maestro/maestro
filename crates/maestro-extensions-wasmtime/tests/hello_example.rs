//! The shipped greeting component loaded into the production engine through the public test
//! harness, as a package test loads its own component.
#![cfg(test)]
use maestro_extensions_wasmtime::test_support::maestro_extensions::{
    AgentToolResult, AgentToolUpdateCallback, Cancellation, Local, wrap_registered_tool,
};
use maestro_extensions_wasmtime::test_support::{Harness, HarnessOptions, Loaded, local};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Recorded arguments, metadata and results the greeting is checked against.
fn corpus() -> Value {
    serde_json::from_str(include_str!("fixtures/hello_expected.json")).unwrap()
}

/// The built greeting component the test run prepared.
fn component() -> String {
    std::env::var("MAESTRO_HELLO_COMPONENT").expect("the run prepares the greeting component")
}

/// A harness whose engine reuses the code the run compiled.
fn harness() -> Harness {
    let options = HarnessOptions {
        agent_dir: maestro_test_process::run_agent_directory().unwrap_or_default(),
        ..HarnessOptions::default()
    };
    Harness::new("/work", options).unwrap()
}

/// Loads the greeting into `harness` and checks that it loaded.
async fn load(harness: &Harness) -> Loaded {
    let loaded = harness.load(vec![component()]).await.unwrap();
    let errors: Vec<_> = loaded.errors.iter().map(|error| &error.error).collect();
    assert!(errors.is_empty(), "{errors:?}");
    loaded
}

/// The caller-visible content and details of one result.
fn result_value(result: &AgentToolResult<Value>) -> Value {
    json!({"content": result.content, "details": result.details})
}

/// One tool registers with the recorded schema and no argument preparation.
#[test]
fn hello_registers_the_greeting_schema() {
    local(async {
        let harness = harness();
        let loaded = load(&harness).await;
        let tools = loaded.runner.get_all_registered_tools();
        assert_eq!(tools.len(), 1);
        let metadata = tools[0].metadata();
        let expected = &corpus()["metadata"];
        assert_eq!(metadata.definition.name, expected["name"]);
        assert_eq!(metadata.label, expected["label"]);
        assert_eq!(metadata.definition.description, expected["description"]);
        assert_eq!(
            serde_json::to_value(&metadata.definition.parameters).unwrap(),
            expected["parameters"]
        );
        let tool = wrap_registered_tool(&tools[0], loaded.runner);
        assert!(tool.prepare_arguments.is_none());
    })
    .unwrap();
}

/// Every supplied string, including empty and whitespace-only names, reaches both outputs.
#[test]
fn hello_preserves_supplied_names() {
    local(async {
        let harness = harness();
        let loaded = load(&harness).await;
        let tools = loaded.runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&tools[0], loaded.runner.clone());
        for row in corpus()["rows"].as_array().unwrap() {
            let result = (tool.execute)("call".into(), row["input"].clone(), None, None)
                .await
                .unwrap();
            assert_eq!(result_value(&result), row["result"], "{}", row["input"]);
        }
    })
    .unwrap();
}

/// An already-aborted signal and a progress callback do not change this tool's answer.
#[test]
fn hello_ignores_unused_invocation_handles() {
    local(async {
        let harness = harness();
        let loaded = load(&harness).await;
        let tools = loaded.runner.get_all_registered_tools();
        let tool = wrap_registered_tool(&tools[0], loaded.runner.clone());
        let signal = Cancellation::new();
        signal.abort();
        let updates = Rc::new(RefCell::new(Vec::new()));
        let observed = updates.clone();
        let update: AgentToolUpdateCallback<Value, Local> =
            Rc::new(move |partial| observed.borrow_mut().push(result_value(&partial)));
        let result = (tool.execute)(
            "another-call".into(),
            json!({"name":"Ada"}),
            Some(signal),
            Some(update),
        )
        .await
        .unwrap();
        assert_eq!(result_value(&result), corpus()["rows"][0]["result"]);
        assert!(updates.borrow().is_empty());
    })
    .unwrap();
}

/// A runtime that a reload replaced refuses the greeting's registration, and the refusal
/// reaches the loader as the component's load failure.
#[test]
fn hello_registration_refused_by_a_replaced_runtime_fails_the_load() {
    local(async {
        let harness = harness();
        harness
            .runtime
            .invalidate(Some("replaced by a reload".into()));
        let loaded = harness.load(vec![component()]).await.unwrap();
        let errors: Vec<_> = loaded
            .errors
            .iter()
            .map(|error| (error.path.clone(), error.error.clone()))
            .collect();
        assert_eq!(
            errors,
            [(
                component(),
                "Failed to load extension: replaced by a reload".to_owned()
            )]
        );
        assert!(loaded.runner.get_all_registered_tools().is_empty());
    })
    .unwrap();
}

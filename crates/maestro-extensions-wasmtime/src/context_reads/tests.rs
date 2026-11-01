//! Real runner reads across the typed thread boundary.
use super::{Endpoint, Owner, Request};
use crate::local_resources::Pump;
use crate::read_imports::ReadEndpoint;
use crate::tests::{model_support, runner, support::bind_prompt};
use maestro_extensions::Local;
use std::rc::Rc;

#[test]
fn model_reads_keep_the_captured_reader_across_rebinding() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        for command in [false, true] {
            captured_models(command).await;
        }
    });
}

/// Uses unequal model identities to reject a fresh lookup through the replacement reader.
async fn captured_models(command: bool) {
    use maestro_extensions::{ExtensionContextActions, ExtensionExecution, types::ProviderActions};
    let runner = runner("/models");
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let mut endpoints = Vec::new();
    for id in ["original", "replacement"] {
        let model = model_support::model(id);
        runner
            .bind_core(
                Local::unbound_actions(),
                ExtensionContextActions {
                    get_model: Rc::new(move || Ok(Some(model.clone()))),
                    ..ExtensionContextActions::default()
                },
                ProviderActions::default(),
            )
            .unwrap();
        let context = if command {
            Owner::Command(runner.create_command_context())
        } else {
            Owner::Base(runner.create_context())
        };
        endpoints.push(registry.context(context).unwrap());
    }
    let worker = std::thread::spawn(move || {
        for (endpoint, id) in endpoints.iter().zip(["original", "replacement"]) {
            let model = endpoint.read(Request::Model).unwrap().unwrap();
            let decoded: serde_json::Value = serde_json::from_str(&model).unwrap();
            assert_eq!(decoded["id"], id);
        }
    });
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
}

#[test]
fn primitive_requests_observe_rebinding_and_retirement() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        for command in [false, true] {
            let runner = runner("/actual-context");
            bind_prompt(&runner, "first prompt");
            let owner = if command {
                Owner::Command(runner.create_command_context())
            } else {
                Owner::Base(runner.create_context())
            };
            let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump.registry().context(owner).unwrap();
            let worker = std::thread::spawn(move || observe(&endpoint));
            for _ in 0..9 {
                assert!(pump.answer_next().await);
            }
            bind_prompt(&runner, "rebound prompt");
            assert!(pump.answer_next().await);
            runner.invalidate(Some("retired context".into()));
            assert!(pump.answer_next().await);
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
        }
    });
}

/// Observes both current operands and catchable retirement text on the Store side.
fn observe(endpoint: &Endpoint) {
    assert_eq!(endpoint.read(Request::Cwd).unwrap(), "/actual-context");
    assert_eq!(
        endpoint.read(Request::SystemPrompt).unwrap(),
        "first prompt"
    );
    assert!(!endpoint.read(Request::HasUi).unwrap());
    assert!(endpoint.read(Request::IsIdle).unwrap());
    assert!(!endpoint.read(Request::HasPendingMessages).unwrap());
    endpoint.read(Request::Abort).unwrap();
    endpoint.read(Request::Shutdown).unwrap();
    assert!(endpoint.read(Request::Usage).unwrap().is_none());
    assert!(endpoint.read(Request::Model).unwrap().is_none());
    assert_eq!(
        endpoint.read(Request::SystemPrompt).unwrap(),
        "rebound prompt"
    );
    assert_eq!(endpoint.read(Request::Cwd).unwrap_err(), "retired context");
}

mod signals;

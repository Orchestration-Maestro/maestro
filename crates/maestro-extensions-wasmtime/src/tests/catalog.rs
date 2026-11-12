//! Catalog capabilities stay live after their acquiring contexts retire.
use super::*;
use crate::read_imports::ReadEndpoint;
use crate::{
    catalog_reads,
    context_reads::{Owner, Request},
    local_resources::Pump,
};
use serde_json::{Value, json};

mod auth;

#[test]
fn retained_catalog_reads_current_models_after_retirement() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        for command in [false, true] {
            retained_catalog(command).await;
        }
    });
}

/// Changes the catalog after acquisition so a frozen descriptor list cannot pass.
async fn retained_catalog(command: bool) {
    let (session, _) = session::runner_session("/catalog");
    let catalog = session.model_registry.clone();
    let list = catalog.get_all();
    list.write().unwrap().clear();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::default()),
        create_extension_runtime::<Local>(),
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
    let owner = if command {
        Owner::Command(runner.create_command_context())
    } else {
        Owner::Base(runner.create_context())
    };
    let endpoint = pump.registry().context(owner).unwrap();
    let worker = std::thread::spawn(move || {
        let catalog = endpoint.read(Request::ModelRegistry).unwrap();
        drop(endpoint);
        observe(&catalog);
    });
    assert!(pump.answer_next().await);
    let model = model_support::model(" \u{feff}\u{85} model ");
    list.write().unwrap().push(model);
    runner.invalidate(Some("retired catalog context".into()));
    drop(runner);
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
    assert_eq!(std::sync::Arc::strong_count(&catalog), 1);
}

/// Exact lookup operands and owned decoding preserve absence and authored strings.
fn observe(catalog: &catalog_reads::Endpoint) {
    let all: Value = serde_json::from_str(&catalog.get_all().unwrap()).unwrap();
    assert_eq!(all.as_array().unwrap().len(), 1);
    assert_eq!(all[0]["id"], " \u{feff}\u{85} model ");
    let available: Value = serde_json::from_str(&text(catalog.get_available())).unwrap();
    assert_eq!(available, json!([]));
    let found = catalog
        .find("fixture".into(), " \u{feff}\u{85} model ".into())
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&found).unwrap(), all[0]);
    assert_eq!(
        catalog.find("fixture".into(), "model".into()).unwrap(),
        None
    );
}

/// The text a finished route settles with.
fn text(mut route: crate::import_route::Route) -> String {
    match route.next() {
        Err(Ok(crate::import_route::Value::Text(text))) => text,
        _ => panic!("expected a text result"),
    }
}

#[test]
fn released_catalog_rejects_every_kind_of_operation_with_its_ended_text() {
    let (session, _) = session::runner_session("/ended");
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump.registry().catalog(session.model_registry).unwrap();
    let lease = endpoint.0.clone();
    let errors = ended_errors(lease, pump, admission, move || ended_operations(&endpoint));
    assert_eq!(errors, ["extension catalog resource ended"; 4]);
}

/// The failure each operation kind settles with: read, route, authentication and provider key.
fn ended_operations(endpoint: &catalog_reads::Endpoint) -> [String; 4] {
    let read = endpoint.get_all().unwrap_err();
    let route = endpoint.refresh().result().unwrap_err();
    let model = model_support::model("ended").read().unwrap().clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let auth = runtime.block_on(endpoint.auth(model)).err().unwrap();
    let key = runtime
        .block_on(endpoint.provider_key("fixture".into()))
        .unwrap_err();
    [read, route, auth, key]
}

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
    use catalog_reads::Request as Read;
    let all: Value = serde_json::from_str(&catalog.read(Read::All).unwrap()).unwrap();
    assert_eq!(all.as_array().unwrap().len(), 1);
    assert_eq!(all[0]["id"], " \u{feff}\u{85} model ");
    let available: Value = serde_json::from_str(&catalog.read(Read::Available).unwrap()).unwrap();
    assert_eq!(available, json!([]));
    let found = catalog
        .read(|reply| Read::Find("fixture".into(), " \u{feff}\u{85} model ".into(), reply))
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&found).unwrap(), all[0]);
    assert_eq!(
        catalog
            .read(|reply| Read::Find("fixture".into(), "model".into(), reply))
            .unwrap(),
        None
    );
}

//! Browser session persistence at the injected storage boundary.
#![cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
use crate::browser_storage_support as storage;
use maestro_app::browser::storage::SessionsStore;
use serde_json::json;

#[test]
fn session_schemas_keep_both_id_indexes() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/browser_sessions.json")).unwrap();
    let expected = &fixture["rows"][0]["expected"]["result"];
    assert_eq!(
        json!({"data": SessionsStore::default().get_config(), "metadata": SessionsStore::get_metadata_config()}),
        *expected
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_records_roundtrip_standard_browser_and_open_custom_history() {
    run_io(async {
        for row in
            session_rows("session_records_roundtrip_standard_browser_and_open_custom_history")
        {
            let backend = storage::Backend::new();
            let store = SessionsStore::default();
            store.set_backend(backend.clone());
            let data: SessionData<BrowserCustom<Open>> =
                serde_json::from_value(row.input["data"].clone()).unwrap();
            let metadata: SessionMetadata =
                serde_json::from_value(row.input["metadata"].clone()).unwrap();
            store.save(&data, &metadata).await.unwrap();
            let loaded: SessionData<BrowserCustom<Open>> =
                store.load_session(&data.id).await.unwrap().unwrap();
            records_equal(
                &serde_json::to_value(&loaded).unwrap(),
                &row.expected["result"]["result"]["loaded"],
            );
            let metadata = store.get_metadata(&data.id).await.unwrap().unwrap();
            records_equal(
                &serde_json::to_value(metadata).unwrap(),
                &row.expected["result"]["result"]["metadata"],
            );
            records_equal(
                &json!(*backend.data.borrow()),
                &row.expected["result"]["records"],
            );
            let agent: Agent<BrowserCustom<Open>> = Agent::new(AgentOptions::default());
            agent
                .state()
                .write()
                .unwrap()
                .set_messages(loaded.messages.read().unwrap().clone());
            let messages = agent.state().read().unwrap().messages().clone();
            let projection = default_convert_to_llm(&messages.read().unwrap());
            records_equal(
                &serde_json::to_value(projection).unwrap(),
                &row.expected["result"]["result"]["replay"],
            );
            records_equal(
                &json!(*backend.trace.borrow()),
                &row.expected["result"]["trace"],
            );
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
use crate::support::run_io;
#[cfg(not(target_arch = "wasm32"))]
use maestro_agent::{Agent, AgentOptions, CustomAgentMessages};
#[cfg(not(target_arch = "wasm32"))]
use maestro_app::browser::{
    components::messages::{CustomAgentMessages as BrowserCustom, default_convert_to_llm},
    storage::{SessionData, SessionMetadata},
};
#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;
/// Open custom record for persisted application messages.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize, Deserialize)]
struct Open {
    /// Role used by the caller.
    role: String,
    /// Open payload fields.
    #[serde(flatten)]
    fields: serde_json::Map<String, Value>,
}
#[cfg(not(target_arch = "wasm32"))]
impl CustomAgentMessages for Open {
    fn role(&self) -> &str {
        &self.role
    }
}
/// Load the strict recorded session envelope.
fn session_rows(name: &str) -> Vec<crate::support::browser_storage_records::Row> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Corpus {
        clock: Vec<String>,
        rows: Vec<crate::support::browser_storage_records::Row>,
    }
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/browser_sessions.json")).unwrap();
    assert_eq!(corpus.rows.len(), 146);
    assert!(!corpus.clock.is_empty());
    let rows: Vec<_> = corpus.rows.into_iter().filter(|r| r.test == name).collect();
    assert!(!rows.is_empty());
    rows
}
#[cfg(not(target_arch = "wasm32"))]
use storage::provider_equal as records_equal;
/// Bound adapter initialized from this case's recorded inputs.
#[cfg(not(target_arch = "wasm32"))]
fn setup(row: &storage::Row) -> (std::rc::Rc<SessionsStore>, std::rc::Rc<storage::Backend>) {
    let store = std::rc::Rc::new(SessionsStore::default());
    let backend = storage::Backend::new();
    if let Some(records) = row.input.get("records") {
        for (name, values) in records.as_object().unwrap() {
            backend.seed(name, values.clone());
        }
    }
    if let Some(index) = row.input.get("index") {
        *backend.index.borrow_mut() = index.as_array().unwrap().clone();
    }
    *backend.fail_at.borrow_mut() = row
        .input
        .get("failAt")
        .map(|v| usize::try_from(v.as_u64().unwrap()).unwrap());
    store.set_backend(backend.clone());
    (store, backend)
}
/// Observe the original cause, transcript and committed records after an operation.
#[cfg(not(target_arch = "wasm32"))]
fn observation<T: Serialize>(result: std::io::Result<T>, backend: &storage::Backend) -> Value {
    let (result, error) = match result {
        Ok(v) => (serde_json::to_value(v).unwrap(), Value::Null),
        Err(e) => {
            let cause = e
                .get_ref()
                .unwrap()
                .downcast_ref::<storage::Cause>()
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(&cause.0, &backend.cause));
            (Value::Null, json!("adapter"))
        }
    };
    json!({"result":result,"error":error,"trace":*backend.trace.borrow(),"records":*backend.data.borrow()})
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_save_uses_one_ordered_two_store_transaction() {
    run_io(async {
        for row in session_rows("session_save_uses_one_ordered_two_store_transaction") {
            let (store, backend) = setup(&row);
            let data: SessionData = serde_json::from_value(row.input["data"].clone()).unwrap();
            let metadata: SessionMetadata =
                serde_json::from_value(row.input["metadata"].clone()).unwrap();
            records_equal(
                &observation(store.save(&data, &metadata).await, &backend),
                &row.expected["result"],
            );
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_delete_and_alias_use_one_ordered_transaction() {
    run_io(async {
        for row in session_rows("session_delete_and_alias_use_one_ordered_transaction") {
            let (store, backend) = setup(&row);
            let id = row.input["id"].as_str().unwrap();
            let result = if row.input["alias"].as_bool().unwrap() {
                store.delete_session(id).await
            } else {
                store.delete(id).await
            };
            records_equal(&observation(result, &backend), &row.expected["result"]);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_metadata_listing_requests_descending_index() {
    reads("session_metadata_listing_requests_descending_index");
}
#[cfg(not(target_arch = "wasm32"))]
/// Exercise public typed reads and listing against their complete observations.
fn reads(name: &str) {
    run_io(async {
        for row in session_rows(name) {
            let (store, backend) = setup(&row);
            let id = row.input.get("id").and_then(Value::as_str).unwrap_or("id");
            let result: std::io::Result<Value> = match row
                .input
                .get("op")
                .and_then(Value::as_str)
                .unwrap_or("getAllMetadata")
            {
                "get" => store.get::<BrowserCustom<Open>>(id).await.map(|v| json!(v)),
                "getMetadata" => store.get_metadata(id).await.map(|v| json!(v)),
                "loadSession" => store
                    .load_session::<BrowserCustom<Open>>(id)
                    .await
                    .map(|v| json!(v)),
                "getAllMetadata" => store.get_all_metadata().await.map(|v| json!(v)),
                _ => panic!("unexpected recorded read"),
            };
            records_equal(&observation(result, &backend), &row.expected["result"]);
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_reads_select_full_data_or_metadata_without_rewriting() {
    reads("session_reads_select_full_data_or_metadata_without_rewriting");
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_read_failures_preserve_phase_and_cause() {
    reads("session_read_failures_preserve_phase_and_cause");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_title_update_skips_each_absent_record_independently() {
    titles("session_title_update_skips_each_absent_record_independently");
}
#[cfg(not(target_arch = "wasm32"))]
/// Title edits observe raw records and all four effect boundaries.
fn titles(name: &str) {
    run_io(async {
        for row in session_rows(name) {
            let (store, backend) = setup(&row);
            let result = store
                .update_title(
                    row.input["id"].as_str().unwrap(),
                    row.input["title"].as_str().unwrap(),
                )
                .await;
            records_equal(&observation(result, &backend), &row.expected["result"]);
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_title_failures_preserve_nontransactional_partial_writes() {
    titles("session_title_failures_preserve_nontransactional_partial_writes");
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_title_update_uses_lookup_key_not_embedded_ids() {
    titles("session_title_update_uses_lookup_key_not_embedded_ids");
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_title_update_preserves_unread_open_record_fields() {
    titles("session_title_update_preserves_unread_open_record_fields");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_quota_and_persistence_delegate_without_policy() {
    run_io(async {
        for row in session_rows("session_quota_and_persistence_delegate_without_policy") {
            let (store, backend) = setup(&row);
            *backend.granted.borrow_mut() = row.input["granted"].as_bool().unwrap();
            let result = if row.input["op"] == "getQuotaInfo" {
                store.get_quota_info().await.map(|q| quota_json(&q))
            } else {
                store.request_persistence().await.map(|v| json!(v))
            };
            records_equal(&observation(result, &backend), &row.expected["result"]);
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
/// Observe adapter quota without recalculating it.
fn quota_json(q: &maestro_app::browser::storage::QuotaInfo) -> Value {
    json!({"usage":q.usage,"quota":q.quota,"percent":q.percent})
}

#[cfg(not(target_arch = "wasm32"))]
/// Construct a real agent retaining the recorded model and complete history.
fn agent_from(value: &Value) -> Agent<BrowserCustom<Open>> {
    use maestro_agent::AgentInitialState;
    Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            model: Some(std::sync::Arc::new(std::sync::RwLock::new(
                serde_json::from_value(value["model"].clone()).unwrap(),
            ))),
            thinking_level: Some(serde_json::from_value(value["thinkingLevel"].clone()).unwrap()),
            messages: serde_json::from_value(value["messages"].clone()).unwrap(),
            ..Default::default()
        },
        ..Default::default()
    })
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_alias_defaults_keep_zero_usage_and_separate_timestamps() {
    aliases("session_alias_defaults_keep_zero_usage_and_separate_timestamps");
}
#[cfg(not(target_arch = "wasm32"))]
/// Observe public writes, qualifying only generated clock fields in the call window.
fn aliases(name: &str) {
    run_io(async {
        for row in session_rows(name) {
            let (store, backend) = setup(&row);
            let agent = agent_from(&row.input["state"]);
            let metadata: Option<SessionMetadata> = row
                .input
                .get("metadata")
                .map(|v| serde_json::from_value(v.clone()).unwrap());
            let start = maestro_models::timestamp_now();
            let result = store
                .save_session(
                    row.input["id"].as_str().unwrap(),
                    agent.state(),
                    metadata.as_ref(),
                    row.input.get("title").and_then(Value::as_str),
                )
                .await;
            let end = maestro_models::timestamp_now();
            let actual = observation(result, &backend);
            let mut expected = row.expected["result"].clone();
            qualify_alias_times(&actual, &mut expected, metadata.is_some(), start, end);
            records_equal(&actual, &expected);
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
/// Generated timestamps are the only corpus instants replaced by observed clock values.
fn qualify_alias_times(actual: &Value, expected: &mut Value, supplied: bool, start: f64, end: f64) {
    let mut times = Vec::new();
    for effect in actual["trace"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect[0] == "tx.set")
    {
        if effect[1] == "sessions" {
            times.push((
                if supplied { 0 } else { 2 },
                effect[3]["lastModified"].as_str().unwrap().to_owned(),
            ));
        }
        if !supplied && effect[1] == "sessions-metadata" {
            times.push((0, effect[3]["createdAt"].as_str().unwrap().to_owned()));
            times.push((1, effect[3]["lastModified"].as_str().unwrap().to_owned()));
        }
    }
    let clock: Value =
        serde_json::from_str(include_str!("fixtures/browser_sessions.json")).unwrap();
    for (index, time) in times {
        let parsed = chrono::DateTime::parse_from_rfc3339(&time).unwrap();
        let millis = num_traits::ToPrimitive::to_f64(&parsed.timestamp_millis()).unwrap();
        assert!(millis >= start.floor() && millis <= end.ceil());
        assert_eq!(time.len(), 24);
        assert!(time.ends_with('Z'));
        replace_instant(expected, clock["clock"][index].as_str().unwrap(), &time);
    }
    if let Some(result) = expected["result"].as_object_mut() {
        result.shift_remove("clockReads");
        if result.is_empty() {
            expected["result"] = Value::Null;
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Replace one independently qualified generated instant wherever persisted.
fn replace_instant(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::String(s) if s == from => *s = to.into(),
        Value::Array(values) => {
            for value in values {
                replace_instant(value, from, to);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_instant(value, from, to);
            }
        }
        _ => {}
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_alias_preserves_supplied_metadata_and_title_precedence() {
    aliases("session_alias_preserves_supplied_metadata_and_title_precedence");
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_alias_save_propagates_transaction_failure() {
    aliases("session_alias_save_propagates_transaction_failure");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn latest_session_keeps_stable_descending_timestamp_order() {
    latest("latest_session_keeps_stable_descending_timestamp_order");
}
#[cfg(not(target_arch = "wasm32"))]
/// Latest selection observes the original listing and its failure boundary.
fn latest(name: &str) {
    run_io(async {
        for row in session_rows(name) {
            let (store, backend) = setup(&row);
            records_equal(
                &observation(store.get_latest_session_id().await, &backend),
                &row.expected["result"],
            );
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn latest_session_propagates_index_failure() {
    latest("latest_session_propagates_index_failure");
}

use maestro_app::browser::storage::get_app_storage;
#[cfg(not(target_arch = "wasm32"))]
use maestro_app::browser::storage::{
    AppStorage, CustomProvidersStore, ProviderKeysStore, SettingsStore, StorageBackend,
    set_app_storage,
};
#[cfg(not(target_arch = "wasm32"))]
use std::rc::Rc;
#[test]
fn app_storage_reports_uninitialized_access() {
    let row = session_rows("app_storage_reports_uninitialized_access").remove(0);
    let error = get_app_storage().err().expect("uninitialized");
    assert_eq!(error.to_string(), row.expected["error"].as_str().unwrap());
    assert_eq!(error.kind(), std::io::ErrorKind::NotConnected);
}
#[cfg(not(target_arch = "wasm32"))]
/// Construct an application without rebinding its independently retained collections.
fn application(backend: Rc<dyn StorageBackend>) -> Rc<AppStorage> {
    Rc::new(AppStorage::new(
        Rc::new(SettingsStore::default()),
        Rc::new(ProviderKeysStore::default()),
        Rc::new(SessionsStore::default()),
        Rc::new(CustomProvidersStore::default()),
        backend,
    ))
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn app_storage_retains_supplied_handles_without_binding() {
    let row = session_rows("app_storage_retains_supplied_handles_without_binding").remove(0);
    let backend = storage::Backend::new();
    let app = application(backend.clone());
    let retained = AppStorage::new(
        app.settings.clone(),
        app.provider_keys.clone(),
        app.sessions.clone(),
        app.custom_providers.clone(),
        app.backend.clone(),
    );
    let unbound = app.settings.get_backend().is_err()
        && app.provider_keys.get_backend().is_err()
        && app.sessions.get_backend().is_err()
        && app.custom_providers.get_backend().is_err();
    records_equal(
        &json!({"sameSettings": Rc::ptr_eq(&retained.settings,&app.settings),"sameKeys":Rc::ptr_eq(&retained.provider_keys,&app.provider_keys),"sameSessions":Rc::ptr_eq(&retained.sessions,&app.sessions),"sameProviders":Rc::ptr_eq(&retained.custom_providers,&app.custom_providers),"sameBackend":Rc::ptr_eq(&retained.backend,&app.backend),"unbound":unbound,"trace":*backend.trace.borrow()}),
        &row.expected["result"],
    );
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn app_storage_replacement_keeps_old_retained_handles() {
    run_io(async {
        let row = session_rows("app_storage_replacement_keeps_old_retained_handles").remove(0);
        let old = storage::Backend::new();
        let new = storage::Backend::new();
        for (backend, key) in [(&old, "old"), (&new, "replacement")] {
            let value = row.input[key].as_f64().unwrap();
            *backend.quota.borrow_mut() = Some(maestro_app::browser::storage::QuotaInfo {
                usage: value,
                quota: 100.0,
                percent: value,
            });
        }
        let old_app = application(old);
        let new_app = application(new);
        set_app_storage(old_app.clone());
        let retained = get_app_storage().unwrap();
        let first = Rc::ptr_eq(&retained, &old_app);
        set_app_storage(new_app.clone());
        let current = get_app_storage().unwrap();
        records_equal(
            &json!({"firstWasOriginal":first,"currentIsReplacement":Rc::ptr_eq(&current,&new_app),"retainedSettingsOriginal":Rc::ptr_eq(&retained.settings,&old_app.settings),"retainedQuota":quota_json(&retained.get_quota_info().await.unwrap()),"currentQuota":quota_json(&current.get_quota_info().await.unwrap())}),
            &row.expected["result"],
        );
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn app_storage_delegates_quota_to_its_own_backend() {
    run_io(async {
        for row in session_rows("app_storage_delegates_quota_to_its_own_backend") {
            let backend = storage::Backend::new();
            let app = application(backend.clone());
            let decoy = storage::Backend::new();
            app.sessions.set_backend(decoy.clone());
            *backend.granted.borrow_mut() = row.input["granted"].as_bool().unwrap();
            *backend.fail_at.borrow_mut() = row
                .input
                .get("failAt")
                .map(|v| usize::try_from(v.as_u64().unwrap()).unwrap());
            let result = if row.input["op"] == "getQuotaInfo" {
                app.get_quota_info().await.map(|q| quota_json(&q))
            } else {
                app.request_persistence().await.map(|v| json!(v))
            };
            let mut actual = observation(result, &backend);
            actual.as_object_mut().unwrap().shift_remove("records");
            records_equal(&actual, &row.expected["result"]);
            assert!(decoy.trace.borrow().is_empty());
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_transactions_keep_live_message_and_model_handles() {
    run_io(async {
        for row in session_rows("session_transactions_keep_live_message_and_model_handles") {
            let (store, backend) = setup(&row);
            let data: SessionData = serde_json::from_value(row.input["data"].clone()).unwrap();
            let metadata: SessionMetadata =
                serde_json::from_value(row.input["metadata"].clone()).unwrap();
            let model = data.model.clone();
            let messages = data.messages.clone();
            let name = row.input["mutation"]["modelName"]
                .as_str()
                .unwrap()
                .to_owned();
            let message = serde_json::from_value(row.input["mutation"]["message"].clone()).unwrap();
            let at = if row.input["phase"] == "before-data-write" {
                1
            } else {
                2
            };
            backend.hooks.borrow_mut().push_back((
                at,
                Box::new(move || {
                    model.write().unwrap().name = name;
                    messages.write().unwrap().push(message);
                }),
            ));
            store.save(&data, &metadata).await.unwrap();
            let loaded: SessionData = store.load_session(&data.id).await.unwrap().unwrap();
            records_equal(
                &json!({"loaded":loaded,"trace":*backend.trace.borrow()}),
                &row.expected["result"],
            );
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_alias_keeps_captured_history_after_state_collection_replacement() {
    run_io(async {
        for row in
            session_rows("session_alias_keeps_captured_history_after_state_collection_replacement")
        {
            let (store, backend) = setup(&row);
            let agent = agent_from(&row.input["state"]);
            let live = agent.state().clone();
            let old = live.read().unwrap().messages().clone();
            let replacement =
                serde_json::from_value(row.input["replacementMessages"].clone()).unwrap();
            let append = serde_json::from_value(row.input["append"].clone()).unwrap();
            backend.hooks.borrow_mut().push_back((
                1,
                Box::new(move || {
                    live.try_write()
                        .expect("save must release its state read guard")
                        .set_messages(replacement);
                    old.write().unwrap().push(append);
                }),
            ));
            let metadata: Option<SessionMetadata> = row
                .input
                .get("metadata")
                .map(|v| serde_json::from_value(v.clone()).unwrap());
            let start = maestro_models::timestamp_now();
            store
                .save_session(
                    row.input["id"].as_str().unwrap(),
                    agent.state(),
                    metadata.as_ref(),
                    None,
                )
                .await
                .unwrap();
            let loaded: SessionData<BrowserCustom<Open>> = store
                .load_session(row.input["id"].as_str().unwrap())
                .await
                .unwrap()
                .unwrap();
            let end = maestro_models::timestamp_now();
            let current = agent.state().read().unwrap().messages().clone();
            let mut actual = observation(Ok(()), &backend);
            actual["result"] = json!({"loaded":loaded,"currentMessages":*current.read().unwrap()});
            let mut expected = row.expected["result"].clone();
            qualify_alias_times(&actual, &mut expected, metadata.is_some(), start, end);
            records_equal(&actual, &expected);
            assert!(agent.state().try_write().is_ok());
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_title_rebinding_selects_backend_at_each_phase() {
    run_io(async {
        let row = session_rows("session_title_rebinding_selects_backend_at_each_phase").remove(0);
        let store = Rc::new(SessionsStore::default());
        let backends: Vec<_> = (0..4).map(|_| storage::Backend::new()).collect();
        let seed =
            session_rows("session_title_update_skips_each_absent_record_independently").remove(0);
        for backend in &backends {
            for (name, values) in seed.input["records"].as_object().unwrap() {
                backend.seed(name, values.clone());
            }
        }
        for (backend, next) in backends.iter().zip(backends.iter().skip(1)) {
            let store = store.clone();
            let next = next.clone();
            backend
                .hooks
                .borrow_mut()
                .push_back((1, Box::new(move || store.set_backend(next))));
        }
        store.set_backend(backends[0].clone());
        store.update_title("id", "rebound").await.unwrap();
        records_equal(
            &json!({"traces":backends.iter().map(|b| b.trace.borrow().clone()).collect::<Vec<_>>(),"metadata":backends[1].data.borrow()["sessions-metadata"]["id"],"data":backends[3].data.borrow()["sessions"]["id"]}),
            &row.expected["result"],
        );
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_transactions_retain_backend_selected_before_callback() {
    run_io(async {
        let row =
            session_rows("session_transactions_retain_backend_selected_before_callback").remove(0);
        let seed = session_rows("session_save_uses_one_ordered_two_store_transaction").remove(0);
        let (store, old) = setup(&seed);
        let new = storage::Backend::new();
        let rebound = new.clone();
        let selected = store.clone();
        old.hooks
            .borrow_mut()
            .push_back((1, Box::new(move || selected.set_backend(rebound))));
        let data: SessionData = serde_json::from_value(seed.input["data"].clone()).unwrap();
        let metadata = serde_json::from_value(seed.input["metadata"].clone()).unwrap();
        store.save(&data, &metadata).await.unwrap();
        records_equal(
            &json!({"oldTrace":*old.trace.borrow(),"newTrace":*new.trace.borrow(),"oldRecords":*old.data.borrow(),"newRecords":*new.data.borrow()}),
            &row.expected["result"],
        );
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_load_retains_backend_selected_before_await() {
    run_io(async {
        let row = session_rows("session_load_retains_backend_selected_before_await").remove(0);
        let store = SessionsStore::default();
        let old = storage::Backend::new();
        let new = storage::Backend::new();
        let (send, receive) = tokio::sync::oneshot::channel();
        *old.read_gate.borrow_mut() = Some(receive);
        store.set_backend(old.clone());
        let read = store.load_session::<BrowserCustom<Open>>("id");
        tokio::pin!(read);
        assert!(
            std::future::Future::poll(
                read.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop()),
            )
            .is_pending()
        );
        store.set_backend(new.clone());
        send.send(row.expected["result"]["result"].clone()).unwrap();
        let loaded = read.await.unwrap().unwrap();
        records_equal(
            &json!({"result":loaded,"trace":*old.trace.borrow(),"newTrace":*new.trace.borrow()}),
            &row.expected["result"],
        );
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_operations_require_a_bound_backend() {
    run_io(async {
        let seed = session_rows("session_save_uses_one_ordered_two_store_transaction").remove(0);
        let data: SessionData = serde_json::from_value(seed.input["data"].clone()).unwrap();
        let metadata = serde_json::from_value(seed.input["metadata"].clone()).unwrap();
        let agent: Agent = Agent::new(AgentOptions::default());
        for row in session_rows("session_operations_require_a_bound_backend") {
            let store = SessionsStore::default();
            let result = match row.input["op"].as_str().unwrap() {
                "save" => store.save(&data, &metadata).await,
                "get" => store
                    .get::<maestro_agent::NoCustomMessages>("id")
                    .await
                    .map(|_| ()),
                "getMetadata" => store.get_metadata("id").await.map(|_| ()),
                "getAllMetadata" => store.get_all_metadata().await.map(|_| ()),
                "delete" => store.delete("id").await,
                "deleteSession" => store.delete_session("id").await,
                "updateTitle" => store.update_title("id", "title").await,
                "getQuotaInfo" => store.get_quota_info().await.map(|_| ()),
                "requestPersistence" => store.request_persistence().await.map(|_| ()),
                "saveSession" => store.save_session("id", agent.state(), None, None).await,
                "loadSession" => store
                    .load_session::<maestro_agent::NoCustomMessages>("id")
                    .await
                    .map(|_| ()),
                "getLatestSessionId" => store.get_latest_session_id().await.map(|_| ()),
                _ => panic!("unexpected operation"),
            };
            assert_eq!(row.expected["error"], "unbound-backend");
            let error = result.expect_err("unbound store");
            assert_eq!(error.kind(), std::io::ErrorKind::NotConnected);
            assert_eq!(error.to_string(), "Storage backend not set");
        }
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn latest_session_uses_host_locale_instead_of_lexical_order() {
    if let Ok(locale) = std::env::var("MAESTRO_SESSION_LOCALE") {
        run_io(async {
            let row = session_rows("latest_session_uses_host_locale_instead_of_lexical_order")
                .into_iter()
                .find(|r| r.input["locale"] == locale)
                .unwrap();
            let (store, backend) = setup(&row);
            assert_eq!(
                json!(store.get_latest_session_id().await.unwrap()),
                row.expected["result"]
            );
            let trace = backend.trace.borrow();
            assert_eq!(trace.len(), 1);
            storage::equal(
                &Value::Array(trace[0].as_array().unwrap()[1..].to_vec()),
                &row.expected["trace"][0],
            );
        });
        return;
    }
    for locale in ["en-US", "sv-SE"] {
        let output = maestro_test_process::TestProcess::current(
            "latest_session_uses_host_locale_instead_of_lexical_order",
        )
        .unwrap()
        .nocapture()
        .env("MAESTRO_SESSION_LOCALE", locale)
        .env("LC_ALL", locale)
        .env("LC_MESSAGES", locale)
        .env("LANG", locale)
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "locale child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

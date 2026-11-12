//! Writable setup capabilities operate on the original session, not a copied reader.
use super::ended_errors;
use crate::{history_reads, local_resources::Pump, writer_reads::Endpoint};
use maestro_session::SessionManager;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[test]
fn writable_setup_and_its_reader_share_the_real_session() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let history = Rc::new(RefCell::new(
                SessionManager::in_memory(Some("/setup")).unwrap(),
            ));
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let writer = pump.registry().writable(history.clone()).unwrap();
            let worker = std::thread::spawn(move || {
                observe(&writer);
                let reader = writer.reader().unwrap();
                drop(writer);
                assert_eq!(reader.get_cwd().unwrap(), "/setup");
            });
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            assert_eq!(Rc::strong_count(&history), 1);
        });
}

/// Exercises real mutations through the Store-side endpoint and reads their stored results.
fn observe(writer: &Endpoint) {
    let reader = writer.reader().unwrap();
    assert!(!writer.is_persisted().unwrap());
    assert_eq!(
        writer
            .new_session(Some(
                r#"{"id":"setup-id","parentSession":"  parent  "}"#.into()
            ))
            .unwrap(),
        None
    );
    assert_eq!(reader.get_session_id().unwrap(), "setup-id");
    let id = writer
        .append_message(json!({"role":"user","content":"hello","timestamp":0}).to_string())
        .unwrap();
    let children: Value = serde_json::from_str(&writer.get_children(id.clone()).unwrap()).unwrap();
    assert_eq!(children, json!([]));
    let context: Value = serde_json::from_str(&writer.build_session_context().unwrap()).unwrap();
    assert_eq!(context["messages"][0]["content"], "hello");
    append_records(writer, &id);
    check_records(&reader);
    branch_records(writer, &reader, id);
}

/// Stores every supported append record using its own native operation.
fn append_records(writer: &Endpoint, id: &str) {
    writer
        .append_thinking_level_change(" raw level ".into())
        .unwrap();
    writer
        .append_model_change(" provider ".into(), " model ".into())
        .unwrap();
    writer
        .append_compaction(
            json!({"summary":"summary", "firstKeptEntryId":id, "tokensBefore":7.5, "details":null})
                .to_string(),
            Some(false),
        )
        .unwrap();
    writer
        .append_custom_entry(" custom ".into(), Some("null".into()))
        .unwrap();
    writer.append_session_info(" name ".into()).unwrap();
    writer
        .append_custom_message_entry(
            json!({"customType":"display", "content":"body", "display":false, "details":{"x":1}})
                .to_string(),
        )
        .unwrap();
    writer
        .append_label_change(id.to_owned(), Some(" label ".into()))
        .unwrap();
}

/// Checks optional metadata, normalization policy and exact literal operands.
fn check_records(reader: &history_reads::Endpoint) {
    let entries: Value = serde_json::from_str(&reader.get_entries().unwrap()).unwrap();
    assert_eq!(entries[1]["thinkingLevel"], " raw level ");
    assert_eq!(entries[2]["provider"], " provider ");
    assert_eq!(entries[3]["details"], Value::Null);
    assert_eq!(entries[3]["fromHook"], false);
    assert_eq!(entries[4]["data"], Value::Null);
    assert_eq!(entries[5]["name"], "name");
    assert_eq!(entries[6]["details"], json!({"x":1}));
}

/// Changes ancestry without copying the retained session.
fn branch_records(writer: &Endpoint, reader: &history_reads::Endpoint, id: String) {
    writer.branch(id.clone()).unwrap();
    assert_eq!(reader.get_leaf_id().unwrap().as_deref(), Some(id.as_str()));
    writer.reset_leaf().unwrap();
    assert_eq!(reader.get_leaf_id().unwrap(), None);
    writer
        .branch_with_summary(
            Some(id.clone()),
            "summary".into(),
            (Some("null".into()), Some(false)),
        )
        .unwrap();
    assert_eq!(writer.create_branched_session(id).unwrap(), None);

    assert!(
        writer
            .branch(" missing ".into())
            .unwrap_err()
            .contains("Entry  missing  not found")
    );
    writer
        .set_session_file("/setup-does-not-write/session.jsonl".into())
        .unwrap();
    // The selected file is resolved: Windows puts it on the working drive.
    let selected = std::path::absolute("/setup-does-not-write/session.jsonl").unwrap();
    assert_eq!(
        reader.get_session_file().unwrap(),
        Some(selected.display().to_string())
    );
    assert!(writer.append_message("{".into()).is_err());
}

#[test]
fn released_writer_rejects_every_kind_of_operation_with_its_ended_text() {
    let history = Rc::new(RefCell::new(
        SessionManager::in_memory(Some("/ended-writer")).unwrap(),
    ));
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let writer = pump.registry().writable(history).unwrap();
    let lease = writer.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        [
            writer.is_persisted().unwrap_err(),
            writer.reset_leaf().unwrap_err(),
            writer.set_session_file("/ended".into()).unwrap_err(),
            writer.new_session(None).unwrap_err(),
            writer.append_message("{}".into()).unwrap_err(),
            writer
                .append_model_change("provider".into(), "id".into())
                .unwrap_err(),
            writer
                .branch_with_summary(None, "summary".into(), (None, None))
                .unwrap_err(),
            writer.reader().err().unwrap(),
        ]
    });
    assert_eq!(errors, ["extension writable session resource ended"; 8]);
}

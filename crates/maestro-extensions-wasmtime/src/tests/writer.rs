//! Writable setup capabilities operate on the original session, not a copied reader.
use crate::read_imports::ReadEndpoint;
use crate::{
    history_reads,
    local_resources::Pump,
    writer_reads::{Endpoint, Request},
};
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
                assert_eq!(reader.read(history_reads::Request::Cwd).unwrap(), "/setup");
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
    assert!(!writer.read(Request::Persisted).unwrap());
    assert_eq!(
        writer
            .read(|reply| Request::NewSession(
                Some(r#"{"id":"setup-id","parentSession":"  parent  "}"#.into()),
                reply
            ))
            .unwrap(),
        None
    );
    assert_eq!(
        reader.read(history_reads::Request::SessionId).unwrap(),
        "setup-id"
    );
    let id = writer
        .read(|reply| {
            Request::Message(
                json!({"role":"user","content":"hello","timestamp":0}).to_string(),
                reply,
            )
        })
        .unwrap();
    let children: Value = serde_json::from_str(
        &writer
            .read(|reply| Request::Children(id.clone(), reply))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(children, json!([]));
    let context: Value =
        serde_json::from_str(&writer.read(Request::BuildContext).unwrap()).unwrap();
    assert_eq!(context["messages"][0]["content"], "hello");
    append_records(writer, &id);
    check_records(&reader);
    branch_records(writer, &reader, id);
}

/// Stores every supported append record using its own native operation.
fn append_records(writer: &Endpoint, id: &str) {
    writer
        .read(|reply| Request::Thinking(" raw level ".into(), reply))
        .unwrap();
    writer
        .read(|reply| Request::Model(" provider ".into(), " model ".into(), reply))
        .unwrap();
    writer.read(|reply| Request::Compaction(json!({"summary":"summary", "firstKeptEntryId":id, "tokensBefore":7.5, "details":null}).to_string(), Some(false), reply)).unwrap();
    writer
        .read(|reply| Request::Custom(" custom ".into(), Some("null".into()), reply))
        .unwrap();
    writer
        .read(|reply| Request::Info(" name ".into(), reply))
        .unwrap();
    writer.read(|reply| Request::CustomMessage(json!({"customType":"display", "content":"body", "display":false, "details":{"x":1}}).to_string(), reply)).unwrap();
    writer
        .read(|reply| Request::Label(id.to_owned(), Some(" label ".into()), reply))
        .unwrap();
}

/// Checks optional metadata, normalization policy and exact literal operands.
fn check_records(reader: &history_reads::Endpoint) {
    let entries: Value =
        serde_json::from_str(&reader.read(history_reads::Request::Entries).unwrap()).unwrap();
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
    writer
        .read(|reply| Request::Branch(id.clone(), reply))
        .unwrap();
    assert_eq!(
        reader
            .read(history_reads::Request::LeafId)
            .unwrap()
            .as_deref(),
        Some(id.as_str())
    );
    writer.read(Request::ResetLeaf).unwrap();
    assert_eq!(reader.read(history_reads::Request::LeafId).unwrap(), None);
    writer
        .read(|reply| {
            Request::Summary(
                Some(id.clone()),
                "summary".into(),
                (Some("null".into()), Some(false)),
                reply,
            )
        })
        .unwrap();
    assert_eq!(
        writer
            .read(|reply| Request::CreateBranched(id, reply))
            .unwrap(),
        None
    );

    assert!(
        writer
            .read(|reply| Request::Branch(" missing ".into(), reply))
            .unwrap_err()
            .contains("Entry  missing  not found")
    );
    writer
        .read(|reply| Request::SetFile("/setup-does-not-write/session.jsonl".into(), reply))
        .unwrap();
    // The selected file is resolved: Windows puts it on the working drive.
    let selected = std::path::absolute("/setup-does-not-write/session.jsonl").unwrap();
    assert_eq!(
        reader.read(history_reads::Request::SessionFile).unwrap(),
        Some(selected.display().to_string())
    );
    assert!(
        writer
            .read(|reply| Request::Message("{".into(), reply))
            .is_err()
    );
}

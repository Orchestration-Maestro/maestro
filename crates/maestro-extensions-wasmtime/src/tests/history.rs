//! Live retained history reads continue after their guarded context retires.
use super::*;
use crate::read_imports::ReadEndpoint;
use crate::{
    context_reads::{Owner, Request},
    history_reads,
    local_resources::Pump,
};
use maestro_session::Presence;
use serde_json::{Value, json};

#[test]
fn label_reply_preserves_domain_string_without_json_quotes() {
    let (_, history) = session::runner_session("/label");
    let id = history
        .borrow_mut()
        .append_custom_entry("label-target", Presence::Missing)
        .unwrap();
    history
        .borrow_mut()
        .append_label_change(&id, Presence::Present(" \u{feff}\u{85} label "))
        .unwrap();
    let (reply, response) = std::sync::mpsc::channel();
    let (pump, _admission) = Pump::new(crate::tests::support::widgets());
    history_reads::Request::Label(id, reply).answer(&*history.borrow(), &pump.registry());
    assert_eq!(
        response.recv().unwrap().unwrap().as_deref(),
        Some(" \u{feff}\u{85} label ")
    );
}

#[test]
fn history_keeps_its_live_owner_after_context_retirement() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        for command in [false, true] {
            retained_history(command).await;
        }
    });
}

/// Captures history before publication, then observes later records through that same owner.
async fn retained_history(command: bool) {
    let (session, history) = session::runner_session("/retained-history");
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
    let (entry_id, received_id) = std::sync::mpsc::channel::<String>();
    let worker = std::thread::spawn(move || {
        let reader = endpoint.read(Request::SessionManager).unwrap();
        drop(endpoint);
        let id = received_id.recv().unwrap();
        observe(&reader, &id);
    });
    assert!(pump.answer_next().await);
    runner.invalidate(Some("retired history context".into()));
    drop(runner);
    let id = history
        .borrow_mut()
        .append_custom_entry(
            "retained",
            Presence::Present(json!({"text":" \u{feff}\u{85} "})),
        )
        .unwrap();
    history
        .borrow_mut()
        .append_label_change(&id, Presence::Present(" \u{feff}\u{85} label "))
        .unwrap();
    history
        .borrow_mut()
        .append_session_info("after retirement")
        .unwrap();
    entry_id.send(id).unwrap();
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
    assert_eq!(
        Rc::strong_count(&history),
        1,
        "released reader retained its owner"
    );
}

/// Covers every non-tree read against real records, with missing operands kept absent.
fn observe(reader: &history_reads::Endpoint, id: &str) {
    use history_reads::Request as Read;
    assert_eq!(reader.read(Read::Cwd).unwrap(), "/retained-history");
    assert_eq!(reader.read(Read::SessionDir).unwrap(), "");
    assert!(!reader.read(Read::SessionId).unwrap().is_empty());
    assert_eq!(reader.read(Read::SessionFile).unwrap(), None);
    assert_eq!(
        reader.read(Read::SessionName).unwrap().as_deref(),
        Some("after retirement")
    );
    observe_entries(reader, id);
}

/// Checks raw records, ancestry and exact labels without adding wire quotes.
fn observe_entries(reader: &history_reads::Endpoint, id: &str) {
    use history_reads::Request as Read;
    let entry = reader
        .read(|reply| Read::Entry(id.into(), reply))
        .unwrap()
        .unwrap();
    let entry: Value = serde_json::from_str(&entry).unwrap();
    assert_eq!(entry["customType"], "retained");
    assert_eq!(entry["data"], json!({"text":" \u{feff}\u{85} "}));
    assert_eq!(
        reader
            .read(|reply| Read::Entry("absent".into(), reply))
            .unwrap(),
        None
    );
    assert_eq!(
        reader
            .read(|reply| Read::Label(id.into(), reply))
            .unwrap()
            .as_deref(),
        Some(" \u{feff}\u{85} label ")
    );
    let entries: Value = serde_json::from_str(&reader.read(Read::Entries).unwrap()).unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 3);
    assert_eq!(entries[0], entry);
    let branch: Value =
        serde_json::from_str(&reader.read(|reply| Read::Branch(None, reply)).unwrap()).unwrap();
    assert_eq!(branch, entries);
    let leaf = reader.read(Read::LeafEntry).unwrap().unwrap();
    assert_eq!(serde_json::from_str::<Value>(&leaf).unwrap(), entries[2]);
    assert_eq!(
        reader.read(Read::LeafId).unwrap().unwrap(),
        entries[2]["id"]
    );
    let header = reader.read(Read::Header).unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&header).unwrap()["cwd"],
        "/retained-history"
    );
}

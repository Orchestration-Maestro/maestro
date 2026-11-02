//! Tree nodes retain one captured tree after its reader and context are released.
use super::*;
use crate::read_imports::ReadEndpoint;
use crate::{
    context_reads::{Owner, Request},
    history_reads,
    local_resources::Pump,
    tree_reads,
};
use maestro_session::Presence;
use serde_json::Value;

#[test]
fn captured_nodes_outlive_reader_without_rebuilding_history() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (session, history) = session::runner_session("/tree");
        let root_id = history
            .borrow_mut()
            .append_custom_entry("root", Presence::Missing)
            .unwrap();
        history
            .borrow_mut()
            .append_label_change(&root_id, Presence::Present("before"))
            .unwrap();
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::default()),
            create_extension_runtime::<Local>(),
            session,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
        let endpoint = pump
            .registry()
            .context(Owner::Base(runner.create_context()))
            .unwrap();
        let observed_id = root_id.clone();
        let worker = std::thread::spawn(move || {
            let history = endpoint.read(Request::SessionManager).unwrap();
            drop(endpoint);
            let roots = history.read(history_reads::Request::Tree).unwrap();
            drop(history);
            assert_eq!(roots.len(), 1);
            observe(&roots[0], &observed_id);
        });
        assert!(pump.answer_next().await);
        // Context release is queued before the tree request.
        assert!(pump.answer_next().await);
        assert!(pump.answer_next().await);
        history
            .borrow_mut()
            .append_label_change(&root_id, Presence::Present("after"))
            .unwrap();
        history.borrow_mut().append_session_info("later").unwrap();
        runner.invalidate(Some("retired tree context".into()));
        drop(runner);
        drop(admission);
        pump.serve().await;
        worker.join().unwrap();
        assert_eq!(Rc::strong_count(&history), 1);
    });
}

/// Repeated child reads preserve the captured contents even after later history appends.
fn observe(root: &tree_reads::Endpoint, root_id: &str) {
    use tree_reads::Request as Read;
    let entry: Value = serde_json::from_str(&root.read(Read::Entry).unwrap()).unwrap();
    assert_eq!(entry["id"], root_id);
    assert_eq!(root.read(Read::Label).unwrap().as_deref(), Some("before"));
    let children = root.read(Read::Children).unwrap();
    assert_eq!(children.len(), 1);
    let child_entry: Value = serde_json::from_str(&children[0].read(Read::Entry).unwrap()).unwrap();
    assert_eq!(child_entry["type"], "label");
    assert_eq!(
        root.read(Read::LabelTimestamp).unwrap(),
        child_entry["timestamp"].as_str().map(str::to_owned)
    );
    assert!(children[0].read(Read::Children).unwrap().is_empty());
    let again = root.read(Read::Children).unwrap();
    assert_eq!(
        again[0].read(Read::Entry).unwrap(),
        children[0].read(Read::Entry).unwrap()
    );
}

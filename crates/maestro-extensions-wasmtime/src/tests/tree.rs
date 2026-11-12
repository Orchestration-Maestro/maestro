//! Tree nodes retain one captured tree after its reader and context are released.
use super::*;
use crate::read_imports::ReadEndpoint;
use crate::{
    context_reads::{Owner, Request},
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
            let roots = history.get_tree().unwrap();
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
    let entry: Value = serde_json::from_str(&root.entry().unwrap()).unwrap();
    assert_eq!(entry["id"], root_id);
    assert_eq!(root.label().unwrap().as_deref(), Some("before"));
    let children = root.children().unwrap();
    assert_eq!(children.len(), 1);
    let child_entry: Value = serde_json::from_str(&children[0].entry().unwrap()).unwrap();
    assert_eq!(child_entry["type"], "label");
    assert_eq!(
        root.label_timestamp().unwrap(),
        child_entry["timestamp"].as_str().map(str::to_owned)
    );
    assert!(children[0].children().unwrap().is_empty());
    let again = root.children().unwrap();
    assert_eq!(again[0].entry().unwrap(), children[0].entry().unwrap());
}

#[test]
fn released_tree_node_rejects_every_kind_of_operation_with_its_ended_text() {
    let (_, history) = session::runner_session("/ended-tree");
    history
        .borrow_mut()
        .append_custom_entry("root", Presence::Missing)
        .unwrap();
    let roots = {
        use maestro_extensions::ReadonlySessionManager as _;
        tree_reads::Owner::capture(history.borrow().get_tree()).unwrap()
    };
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let node = pump
        .registry()
        .node(roots.into_iter().next().unwrap())
        .unwrap();
    let lease = node.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        [
            node.entry().unwrap_err(),
            node.label().unwrap_err(),
            node.label_timestamp().unwrap_err(),
            node.children().err().unwrap(),
        ]
    });
    assert_eq!(errors, ["extension tree node resource ended"; 4]);
}

//! Ordering, identity and lifecycle of the per-file mutation queue.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::read_support;
use crate::write_support;

use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use maestro_tools::with_file_mutation_queue as queue;
use read_support::{Fixture, Scratch};
use serde_json::json;
use std::task::Poll;
use tokio::sync::oneshot;
use write_support::{Gate, Journal, poll_once, ready, rows, single_expected};

/// A queued operation that journals its start, waits for `gate` when given, then journals its end.
fn tracked(
    journal: &Journal,
    name: &'static str,
    gate: Option<oneshot::Receiver<()>>,
) -> impl FnOnce() -> BoxFuture<Result<(), DiagnosticErrorInfo>> + Send + 'static {
    let journal = journal.clone();
    move || {
        Box::pin(async move {
            journal.push(format!("{name}:start"));
            if let Some(gate) = gate {
                gate.await.unwrap();
            }
            journal.push(format!("{name}:end"));
            Ok(())
        })
    }
}

/// The expected journal of a single-row group.
fn expected_journal(test: &str) -> Vec<String> {
    serde_json::from_value(single_expected(test)).unwrap()
}

#[test]
fn mutation_queue_keeps_invocation_order_before_waiting() {
    let dir = Scratch::new().unwrap();
    let journal = Journal::default();
    let (open_a, gate_a) = Gate::new();
    let first = format!("{}/same.txt", dir.text());
    let dotted = format!("{}/missing-dir/../same.txt", dir.text());
    let mut a = queue(&first, tracked(&journal, "a", Some(gate_a)));
    let mut b = queue(&dotted, tracked(&journal, "b", None));
    assert!(poll_once(&mut b).is_pending());
    assert!(journal.entries().is_empty());
    assert!(poll_once(&mut a).is_pending());
    assert_eq!(journal.entries(), ["a:start"]);
    open_a.open();
    ready(&mut a).unwrap();
    ready(&mut b).unwrap();
    assert_eq!(
        journal.entries(),
        expected_journal("mutation_queue_keeps_invocation_order_before_waiting")
    );
}

#[test]
fn mutation_queue_different_paths_can_finish_out_of_order() {
    let dir = Scratch::new().unwrap();
    let journal = Journal::default();
    let (open_a, gate_a) = Gate::new();
    let mut a = queue(
        &format!("{}/a.txt", dir.text()),
        tracked(&journal, "a", Some(gate_a)),
    );
    let mut b = queue(
        &format!("{}/b.txt", dir.text()),
        tracked(&journal, "b", None),
    );
    assert!(poll_once(&mut a).is_pending());
    ready(&mut b).unwrap();
    open_a.open();
    ready(&mut a).unwrap();
    assert_eq!(
        journal.entries(),
        expected_journal("mutation_queue_different_paths_can_finish_out_of_order")
    );
}

#[cfg(unix)]
#[test]
fn mutation_queue_serializes_existing_symlink_aliases() {
    let dir = Scratch::new().unwrap();
    let target = dir.file("target.txt", b"x");
    let alias = format!("{}/alias.txt", dir.text());
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    let journal = Journal::default();
    let (open_a, gate_a) = Gate::new();
    let mut a = queue(&target, tracked(&journal, "a", Some(gate_a)));
    let mut b = queue(&alias, tracked(&journal, "b", None));
    assert!(poll_once(&mut a).is_pending());
    assert!(poll_once(&mut b).is_pending());
    assert_eq!(journal.entries(), ["a:start"]);
    open_a.open();
    ready(&mut a).unwrap();
    ready(&mut b).unwrap();
    assert_eq!(
        journal.entries(),
        expected_journal("mutation_queue_serializes_existing_symlink_aliases")
    );
}

/// Names the raw-identity rows spell inside the test directory; the others stay relative.
const IN_DIRECTORY: [&str; 5] = [
    "target.txt",
    "alias.txt",
    "hard.txt",
    "missing",
    "child/../missing",
];

#[test]
fn mutation_queue_raw_names_preserve_literal_path_identity() {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        pair: [String; 2],
    }
    for row in rows("mutation_queue_raw_names_preserve_literal_path_identity") {
        let dir = Scratch::new().unwrap();
        let target = dir.file("target.txt", b"x");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, dir.path().join("alias.txt")).unwrap();
        std::fs::hard_link(&target, dir.path().join("hard.txt")).unwrap();
        let [first, second] = serde_json::from_value::<Input>(row.input).unwrap().pair;
        if cfg!(not(unix)) && (first == "alias.txt" || second == "alias.txt") {
            // Only Unix fixtures create the symbolic link this pair names.
            continue;
        }
        let spell = |name: &str| {
            if IN_DIRECTORY.contains(&name) {
                format!("{}/{name}", dir.text())
            } else {
                name.to_owned()
            }
        };
        let journal = Journal::default();
        let (open_a, gate_a) = Gate::new();
        let mut a = queue(&spell(&first), tracked(&journal, "a", Some(gate_a)));
        let mut b = queue(&spell(&second), tracked(&journal, "b", None));
        assert!(poll_once(&mut a).is_pending());
        let shared = match poll_once(&mut b) {
            Poll::Pending => true,
            Poll::Ready(result) => {
                result.unwrap();
                false
            }
        };
        assert_eq!(json!(shared), row.expected, "pair {first} / {second}");
        open_a.open();
        ready(&mut a).unwrap();
        if shared {
            ready(&mut b).unwrap();
        }
    }
}

#[test]
fn mutation_queue_failure_does_not_poison_following_work() {
    let failure = DiagnosticErrorInfo {
        name: Some("ControlledFailure".into()),
        message: "first failure".into(),
        stack: Some("controlled stack".into()),
        code: Some(maestro_models::DiagnosticCode::Text("E_CONTROL".into())),
    };
    let dir = Scratch::new().unwrap();
    let path = format!("{}/f.txt", dir.text());
    let (open_a, gate_a) = Gate::new();
    let expected_error = failure.clone();
    let mut a = queue(&path, move || async move {
        gate_a.await.unwrap();
        Err::<&str, _>(failure)
    });
    let mut b = queue(&path, || async { Ok("second success") });
    assert!(poll_once(&mut a).is_pending());
    assert!(poll_once(&mut b).is_pending());
    open_a.open();
    let error = ready(&mut a).unwrap_err();
    assert_eq!(error, expected_error);
    let outcome = [error.message, ready(&mut b).unwrap().to_owned()];
    let expected = expected_journal("mutation_queue_failure_does_not_poison_following_work");
    assert_eq!(outcome.to_vec(), expected);
    let mut later = queue(&path, || async { Ok(()) });
    ready(&mut later).unwrap();
}

#[test]
fn mutation_queue_dropped_middle_waiter_preserves_active_head() {
    let dir = Scratch::new().unwrap();
    let path = format!("{}/m.txt", dir.text());
    let journal = Journal::default();
    let (open_a, gate_a) = Gate::new();
    let mut a = queue(&path, tracked(&journal, "a", Some(gate_a)));
    assert!(poll_once(&mut a).is_pending());
    let b = queue(&path, tracked(&journal, "b", None));
    let mut c = queue(&path, tracked(&journal, "c", None));
    drop(b);
    assert!(poll_once(&mut c).is_pending());
    assert_eq!(journal.entries(), ["a:start"]);
    open_a.open();
    ready(&mut a).unwrap();
    ready(&mut c).unwrap();
    assert_eq!(journal.entries(), ["a:start", "a:end", "c:start", "c:end"]);

    let (open_d, gate_d) = Gate::new();
    let mut d = queue(&path, tracked(&journal, "d", Some(gate_d)));
    assert!(poll_once(&mut d).is_pending());
    let mut waiting_last = queue(&path, tracked(&journal, "e", None));
    assert!(poll_once(&mut waiting_last).is_pending());
    drop(waiting_last);
    open_d.open();
    ready(&mut d).unwrap();

    let unstarted_head = queue(&path, tracked(&journal, "f", None));
    drop(unstarted_head);
    let mut next = queue(&path, tracked(&journal, "g", None));
    ready(&mut next).unwrap();
    let events = journal.entries();
    assert!(
        events
            .iter()
            .all(|event| !event.starts_with(['b', 'e', 'f']))
    );
    assert_eq!(&events[4..], ["d:start", "d:end", "g:start", "g:end"]);
}

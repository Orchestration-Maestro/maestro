//! Shared deleted-text history and owned undo snapshots.
use maestro_tui::kill_ring::{KillRing, KillRingOptions};

#[test]
fn kill_ring_ignores_empty_accumulates_and_rotates() {
    for (prepend, accumulate) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut ring = KillRing::new();
        let options = || KillRingOptions {
            prepend,
            accumulate,
        };
        ring.push("", options());
        assert_eq!(ring.length(), 0);
        ring.rotate();
        assert_eq!(ring.peek(), None);
        ring.push("middle", options());
        ring.rotate();
        assert_eq!(ring.peek(), Some("middle"));
        ring.push("edge", options());
        let expected = match (prepend, accumulate) {
            (_, false) => "edge",
            (true, true) => "edgemiddle",
            (false, true) => "middleedge",
        };
        assert_eq!(ring.peek(), Some(expected));
        assert_eq!(ring.length(), if accumulate { 1 } else { 2 });
        ring.push("", options());
        assert_eq!(ring.peek(), Some(expected));
    }
    let mut ring = KillRing::default();
    for text in ["first", "second", "third"] {
        ring.push(
            text,
            KillRingOptions {
                prepend: false,
                accumulate: false,
            },
        );
    }
    for expected in ["second", "first", "third", "second"] {
        ring.rotate();
        assert_eq!(ring.peek(), Some(expected));
        assert_eq!(ring.length(), 3);
    }
}

#[test]
fn undo_stack_detaches_snapshots_and_clears_lifo() {
    use maestro_tui::undo_stack::UndoStack;
    let mut stack = UndoStack::new();
    let mut snapshot = vec![vec![String::from("original")]];
    stack.push(&snapshot);
    snapshot[0][0].push_str(" changed");
    stack.push(&snapshot);
    snapshot.clear();
    assert_eq!(stack.length(), 2);
    let mut popped = stack.pop().unwrap();
    assert_eq!(popped, vec![vec!["original changed"]]);
    popped[0][0].clear();
    assert_eq!(stack.pop(), Some(vec![vec![String::from("original")]]));
    assert_eq!(stack.pop(), None);
    stack.push(&popped);
    stack.clear();
    assert_eq!(stack.length(), 0);
    assert_eq!(stack.pop(), None);
    let unbounded: UndoStack<NoDefault> = UndoStack::default();
    assert_eq!(unbounded.length(), 0);
}

/// A snapshot type with no default constructor.
struct NoDefault;

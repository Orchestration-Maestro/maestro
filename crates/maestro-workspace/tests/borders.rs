//! Border flag sets compose edge by edge.
#![cfg(test)]
use maestro_workspace::layout::Borders;

#[test]
fn a_set_contains_a_subset_only_when_every_edge_is_set() {
    assert!(Borders::ALL.contains(Borders::TOP.union(Borders::LEFT)));
    assert!(Borders::TOP.contains(Borders::TOP));
    assert!(Borders::TOP.contains(Borders::NONE));
    assert!(!Borders::TOP.contains(Borders::LEFT));
    assert!(!Borders::TOP.contains(Borders::TOP.union(Borders::LEFT)));
}

#[test]
fn only_the_empty_set_is_empty() {
    assert!(Borders::NONE.is_empty());
    assert!(Borders::default().is_empty());
    for edge in [
        Borders::TOP,
        Borders::RIGHT,
        Borders::BOTTOM,
        Borders::LEFT,
        Borders::ALL,
    ] {
        assert!(!edge.is_empty());
    }
}

#[test]
fn union_adds_edges_without_dropping_any() {
    let top_left = Borders::TOP.union(Borders::LEFT);
    assert!(top_left.contains(Borders::TOP) && top_left.contains(Borders::LEFT));
    assert!(!top_left.contains(Borders::RIGHT) && !top_left.contains(Borders::BOTTOM));
    assert_eq!(
        Borders::TOP
            .union(Borders::RIGHT)
            .union(Borders::BOTTOM)
            .union(Borders::LEFT),
        Borders::ALL
    );
    assert_eq!(Borders::ALL.union(Borders::TOP), Borders::ALL);
}

#[test]
fn without_removes_only_the_named_edges() {
    let shared = Borders::ALL.without(Borders::RIGHT);
    assert!(shared.contains(Borders::TOP.union(Borders::BOTTOM).union(Borders::LEFT)));
    assert!(!shared.contains(Borders::RIGHT));
    assert_eq!(Borders::LEFT.without(Borders::RIGHT), Borders::LEFT);
    assert_eq!(Borders::NONE.without(Borders::ALL), Borders::NONE);
    assert_eq!(Borders::ALL.without(Borders::ALL), Borders::NONE);
}

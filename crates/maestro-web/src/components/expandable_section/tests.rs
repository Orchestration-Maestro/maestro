//! Portable preparation retains the caller's opaque child owner.
use super::ExpandableSection;
use std::{cell::Cell, rc::Rc};

/// A child with no cloning, serialization or thread-transport contract.
struct Child(Rc<Cell<u32>>);

#[test]
fn expandable_section_preparation_retains_original_child_owner() {
    let value = Rc::new(Cell::new(7));
    let weak = Rc::downgrade(&value);
    let child = Child(Rc::clone(&value));
    assert!(Rc::ptr_eq(&child.0, &value));
    let mut section = ExpandableSection::new(vec![Box::new(child)]);
    assert!(section.summary.is_empty());
    assert!(!section.default_expanded);
    section.summary = "Literal <b>".into();
    section.default_expanded = true;
    value.set(19);
    drop(value);
    assert_eq!(weak.upgrade().unwrap().get(), 19);
    assert_eq!(section.summary, "Literal <b>");
    drop(section);
    assert!(weak.upgrade().is_none());
}

//! Selected difference semantics preserve unchanged native text nodes.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{container, select};
use maestro_web::{
    components::diff::{DiffProps, diff},
    dom::MountedContent,
};
use wasm_bindgen::JsCast as _;

/// A semantic-equivalence pair with a stable diagnostic label.
type Case = (&'static str, DiffProps, DiffProps, bool);

/// Supply all authored operands without a hidden presentation fallback.
fn props(old: &str, new: &str, title: Option<&str>, class: &str) -> DiffProps {
    DiffProps {
        old_text: old.into(),
        new_text: new.into(),
        title: title.map(str::to_owned),
        class_name: class.into(),
    }
}

/// Equivalent spellings of the same selected line content.
fn equivalences() -> [Case; 6] {
    let plain = |old, new| props(old, new, None, "");
    [
        ("unchanged-input", plain("a", "a"), plain("a", "a"), true),
        (
            "empty-title",
            plain("a", "a"),
            props("a", "a", Some(""), ""),
            true,
        ),
        ("context-lf", plain("a", "a"), plain("a\n", "a\n"), true),
        ("added-lf", plain("", "a"), plain("", "a\n"), true),
        ("removed-lf", plain("a", ""), plain("a\n", ""), true),
        (
            "replacement-lf",
            plain("old", "new"),
            plain("old\n", "new\n"),
            true,
        ),
    ]
}

/// Literal and ordering distinctions that normalization must retain.
fn distinctions() -> [Case; 6] {
    let plain = |old, new| props(old, new, None, "");
    [
        (
            "blank-line",
            plain("a\n", "a\n"),
            plain("a\n\n", "a\n\n"),
            false,
        ),
        (
            "carriage-return",
            plain("a\n", "a\n"),
            plain("a\r\n", "a\r\n"),
            false,
        ),
        (
            "nonempty-title",
            plain("a", "a"),
            props("a", "a", Some("title"), ""),
            false,
        ),
        (
            "whitespace-title",
            plain("a", "a"),
            props("a", "a", Some(" "), ""),
            false,
        ),
        (
            "literal-class",
            plain("a", "a"),
            props("a", "a", None, "custom"),
            false,
        ),
        (
            "run-order",
            plain("a\na", "a"),
            plain("a\na\n", "a\n"),
            false,
        ),
    ]
}

/// Equivalent selected content leaves native descendants untouched; distinct input is visible.
pub fn diff_factory_identity_uses_normalized_semantics(view: &maestro_web::dom::DomView) {
    let parent = container();
    for (name, before, after, equivalent) in equivalences().into_iter().chain(distinctions()) {
        let mut slot = MountedContent::default();
        slot.show(diff(before).as_ref(), &parent, view).unwrap();
        let root = slot.root().unwrap().clone();
        let element: &web_sys::Element = root.unchecked_ref();
        let markup = element.outer_html();
        let first = select(element, ".maestro-diff-scroll pre")
            .first_child()
            .unwrap();
        slot.show(diff(after).as_ref(), &parent, view).unwrap();
        assert!(slot.root().unwrap().is_same_node(Some(&root)), "{name}");
        assert_eq!(element.outer_html() == markup, equivalent, "{name}");
        if equivalent {
            let current = select(element, ".maestro-diff-scroll pre")
                .first_child()
                .unwrap();
            assert!(current.is_same_node(Some(&first)), "{name}");
        }
        slot.clear();
    }
    parent.remove();
}

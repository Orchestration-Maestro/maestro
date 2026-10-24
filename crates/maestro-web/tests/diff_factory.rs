//! Native reading identity follows selected diff content rather than authored spelling.
#![cfg(test)]
use crate::{
    attachment_support::{Frame, Host, reading_factory},
    document_card_support::frame_owner,
    support::tool_output::TextOnlyOutput as _,
};
use maestro_web::{
    components::changes::{DiffProps, diff},
    tools::types::{ToolRenderResult, ToolRenderState},
};
use std::rc::Rc;

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

/// Observe the actual factory offered on each complete render of one retained row.
fn reuses_factory(before: DiffProps, after: DiffProps) -> bool {
    let host = Rc::new(Host::default());
    let mut frame = Frame::default();
    let mut state = ToolRenderState::default();
    let [first, second] = [before, after].map(|props| {
        let recipe = ToolRenderResult {
            content: diff(props),
            is_custom: true,
        };
        let preceding = host.surfaces.borrow().len();
        frame_owner(&mut frame, &host, vec![], |ui, view| {
            recipe.render(ui, view, &mut state)
        })
        .assert_no_overlays()
        .unwrap();
        assert!(host.surfaces.borrow().len() > preceding);
        reading_factory(&host)
    });
    Rc::ptr_eq(&first, &second)
}

#[test]
fn diff_factory_identity_uses_normalized_semantics() {
    let mismatches: Vec<_> = equivalences()
        .into_iter()
        .chain(distinctions())
        .filter_map(|(name, before, after, expected)| {
            (reuses_factory(before, after) != expected).then_some(name)
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "factory identity cases: {mismatches:?}"
    );
}

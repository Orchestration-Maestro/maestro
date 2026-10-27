//! Recorded section moments and release of the captured child's owner.
use crate::contributions_support::{body, select, text};
use maestro_web::{
    ExpandableSection,
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// One recorded section case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lifecycle {
    /// Default expansion applied after the child is captured.
    initial: bool,
    /// Observation after each step.
    expected: Moments,
}
/// Observations after each recorded step.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Moments {
    /// After reconnection applies the initial default.
    first: Moment,
    /// After one toggle.
    toggled: Moment,
    /// After the summary, default and child text change.
    updated: Moment,
    /// After a second toggle.
    restored: Moment,
    /// After reconnection.
    reconnected: Moment,
    /// After a second reconnection.
    reconnected_again: Moment,
}
/// What the page shows at one step.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Moment {
    /// Literal summary.
    text: String,
    /// Shown children.
    children: usize,
    /// Whether the shown child is the originally captured element.
    identity: bool,
    /// The child's current command.
    child_value: String,
    /// Chevron path markup.
    chevron: String,
}

/// A caller-owned console child whose `owner` lives as long as the section holds it.
struct Child {
    /// Owner whose release the test observes.
    owner: Rc<RefCell<String>>,
}
impl DomContent for Child {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = view.document.create_element("div").unwrap();
        root.set_class_name("maestro-console");
        let pre = view.document.create_element("pre").unwrap();
        pre.set_text_content(Some(&format!("> {}", self.owner.borrow())));
        root.append_child(&pre).unwrap();
        Ok(DomRoot::new(root))
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}

/// Compare the mounted section with one recorded moment.
fn observe(root: &Element, original: &Element, moment: &Moment) {
    let summary = select(root, ".maestro-expandable-toggle");
    assert_eq!(text(&summary), moment.text);
    let children = root.query_selector_all(".maestro-console").unwrap();
    assert_eq!(children.length() as usize, moment.children);
    let shown = root.query_selector(".maestro-console").unwrap();
    assert_eq!(
        shown
            .as_ref()
            .is_some_and(|shown| shown.is_same_node(Some(original))),
        moment.identity
    );
    if let Some(shown) = shown {
        assert_eq!(text(&shown), format!("> {}", moment.child_value));
    }
    let chevron = select(root, ".maestro-expandable-chevron").inner_html();
    assert!(chevron.contains(&moment.chevron), "{chevron}");
}

/// Both recorded cases: reconnection, toggles, summary and default changes and a caller edit of
/// the captured child keep the recorded text, children, identity and chevron.
pub(super) fn recorded(view: &DomView) {
    let cases: Vec<Lifecycle> =
        serde_json::from_str(include_str!("../fixtures/expandable_sections.json")).unwrap();
    assert_eq!(cases.len(), 2);
    for case in cases {
        let owner = Rc::new(RefCell::new("retained".to_owned()));
        let mut section = ExpandableSection::new(vec![Box::new(Child {
            owner: Rc::clone(&owner),
        })]);
        section.summary = "Summary <&> 雪".into();
        section.default_expanded = true;
        let root = section.mount(view).unwrap();
        body().append_child(&root.node).unwrap();
        let element: &Element = root.node.unchecked_ref();
        let original = select(element, ".maestro-console");
        let toggle = || {
            select(element, ".maestro-expandable-toggle")
                .unchecked_into::<HtmlElement>()
                .click();
        };
        section.default_expanded = case.initial;
        section.connected_callback(&root).unwrap();
        observe(element, &original, &case.expected.first);
        toggle();
        observe(element, &original, &case.expected.toggled);
        section.summary = "Changed".into();
        section.default_expanded = !case.initial;
        select(&original, "pre").set_text_content(Some("> edited"));
        section.update(&root, view).unwrap();
        observe(element, &original, &case.expected.updated);
        toggle();
        observe(element, &original, &case.expected.restored);
        section.connected_callback(&root).unwrap();
        observe(element, &original, &case.expected.reconnected);
        section.connected_callback(&root).unwrap();
        observe(element, &original, &case.expected.reconnected_again);
        element.remove();
    }
}

/// Collapsing detaches the captured child without releasing it; dropping the section releases
/// its owner, and the detached summary then changes nothing.
pub(super) fn release(view: &DomView) {
    let owner = Rc::new(RefCell::new("captured".to_owned()));
    let released = Rc::downgrade(&owner);
    let mut section = ExpandableSection::new(vec![Box::new(Child { owner })]);
    section.summary = "release".into();
    section.default_expanded = true;
    let root = section.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let element: &Element = root.node.unchecked_ref();
    let child = select(element, ".maestro-console");
    let summary: HtmlElement = select(element, ".maestro-expandable-toggle").unchecked_into();
    summary.click();
    assert!(!child.is_connected());
    assert!(released.upgrade().is_some());
    drop(section);
    assert!(
        released.upgrade().is_none(),
        "the section retained its captured child"
    );
    element.remove();
    summary.click();
    assert!(body().query_selector(".maestro-console").unwrap().is_none());
}

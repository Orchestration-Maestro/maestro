//! Recorded document cards observed through their mounted page roots.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{Row, fixture, prepare};
use crate::contributions_support::{body, host, select, text, view};
use maestro_web::dom::{DomView, MountedContent};
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// One placed card, removed together with its native resource owners.
pub struct Card {
    /// Supplied page operands.
    pub view: Rc<DomView>,
    /// Independent placement for this card.
    pub parent: Element,
    /// The current native content owner.
    pub slot: MountedContent,
}
impl Card {
    /// Mount the actual public tool dispatcher's prepared output.
    pub fn new(view: Rc<DomView>, row: &Row) -> Self {
        let parent = view.document.create_element("section").unwrap();
        let presentation = view.pack.presentation(view.mode).unwrap();
        for (name, value) in presentation.css_properties() {
            parent
                .unchecked_ref::<HtmlElement>()
                .style()
                .set_property(&name, &value)
                .unwrap();
        }
        body().append_child(&parent).unwrap();
        let mut card = Self {
            view,
            parent,
            slot: MountedContent::default(),
        };
        card.show(row);
        card
    }
    /// Refresh this placement without replacing an unchanged content type.
    pub fn show(&mut self, row: &Row) {
        let recipe = prepare(row).unwrap();
        assert!(!recipe.is_custom);
        self.slot
            .show(recipe.content.as_ref(), &self.parent, &self.view)
            .unwrap();
    }
    /// Current card root.
    pub fn root(&self) -> Element {
        self.slot.root().unwrap().clone().unchecked_into()
    }
    /// Activate the native disclosure button.
    pub fn toggle(&self) {
        self.root()
            .first_element_child()
            .unwrap()
            .unchecked_into::<HtmlElement>()
            .click();
    }
}
impl Drop for Card {
    fn drop(&mut self) {
        self.slot.clear();
        self.parent.remove();
    }
}
/// Replay selected original records, including errors in reached malformed metadata.
pub fn rows(import: &js_sys::Function, choose: impl Fn(&Row) -> bool) {
    let view = view();
    let _elements = host(&view, import);
    let rows: Vec<_> = fixture().into_iter().filter(choose).collect();
    assert!(!rows.is_empty());
    for row in rows {
        if let Some(language) = &row.input.language {
            view.i18n.set_language(language).unwrap();
        }
        observe(Rc::clone(&view), &row);
    }
}
/// Compare status, literal text, body ordering and attached file controls with the original oracle.
fn observe(view: Rc<DomView>, row: &Row) {
    let Some(expected) = &row.expected else {
        assert!(prepare(row).is_err(), "{}", row.id);
        return;
    };
    let card = Card::new(view, row);
    let root = card.root();
    status(&root, row);
    contents(&root, expected, &row.id);
    let tiles = root
        .query_selector_all(".maestro-tool-files [data-open]")
        .unwrap();
    assert_eq!(tiles.length() as usize, expected.attachments.len());
    assert!(
        root.query_selector(".maestro-tool-body [data-open]")
            .unwrap()
            .is_none()
    );
    assert!(root.query_selector("script").unwrap().is_none());
    drop(card);
    assert!(!root.is_connected());
}

/// Compare caption, disclosure choice and native status indicators.
fn status(root: &Element, row: &Row) {
    let expected = row.expected.as_ref().unwrap();
    assert_eq!(
        text(&select(root, ".maestro-tool-caption")),
        expected.caption,
        "{}",
        row.id
    );
    assert_eq!(
        crate::contributions_support::disclosure(&root.first_element_child().unwrap()).is_some(),
        expected.collapsible
    );
    assert_eq!(
        root.query_selector_all(".maestro-tool-spinner")
            .unwrap()
            .length() as usize,
        expected.spinner
    );
    for status in ["complete", "error"] {
        assert_eq!(
            root.query_selector(&format!(".maestro-tool-{status}"))
                .unwrap()
                .is_some(),
            expected.status == status,
            "{}",
            row.id
        );
    }
}
/// Compare the complete original URL and ordered literal outputs.
fn contents(root: &Element, expected: &super::Expected, id: &str) {
    let url = root
        .query_selector(".maestro-tool-url")
        .unwrap()
        .map(|url| text(&url));
    assert_eq!(url, expected.url, "{id}");
    listings(root, &expected.blocks, id);
    consoles(root, &expected.console, id);
}
/// Every code listing keeps its original order and language.
fn listings(root: &Element, expected: &[super::Block], id: &str) {
    let blocks = root.query_selector_all(".maestro-code").unwrap();
    assert_eq!(blocks.length() as usize, expected.len());
    for (index, expected) in expected.iter().enumerate() {
        let block = blocks.item(u32::try_from(index).unwrap()).unwrap();
        assert_eq!(text(&select(&block, "pre code")), expected.code, "{id}");
        assert_eq!(
            block
                .unchecked_ref::<Element>()
                .get_attribute("language")
                .unwrap(),
            expected.language
        );
    }
}
/// Console text and destructive styling follow the selected output branch.
fn consoles(root: &Element, expected: &[super::Console], id: &str) {
    let consoles = root.query_selector_all(".maestro-console").unwrap();
    assert_eq!(consoles.length() as usize, expected.len());
    for (index, expected) in expected.iter().enumerate() {
        let console = consoles.item(u32::try_from(index).unwrap()).unwrap();
        assert_eq!(text(&select(&console, "pre")), expected.content, "{id}");
        assert_eq!(
            console
                .unchecked_ref::<Element>()
                .query_selector(".maestro-console-error")
                .unwrap()
                .is_some(),
            expected.variant == "error"
        );
    }
}

//! Console frame nodes shared by canvas drawing and native page mounting.
use super::ConsoleVariant;
use crate::browser::attachment_content::{
    AttachmentHostError,
    browser::{append, element, set_text_if_changed},
};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// A console's frame: its root, localized header label and scrolling literal body.
pub(super) struct Nodes {
    /// Root carrying the console's own stylesheet.
    pub(super) root: Element,
    /// Header receiving the caller's own control after the label.
    pub(super) header: Element,
    /// Localized header label.
    pub(super) label: Element,
    /// Feature-owned scrolling body, distinct from the host viewport.
    pub(super) scroll: HtmlElement,
    /// Literal output body.
    pub(super) pre: Element,
}

impl Nodes {
    /// Build the frame with its stylesheet, label and literal body in place.
    pub(super) fn build(document: &web_sys::Document) -> Result<Self, AttachmentHostError> {
        let root = element(document, "div", "maestro-console")?;
        let style = element(document, "style", "")?;
        style.set_text_content(Some(include_str!("console.css")));
        append(&root, &style)?;
        let header = element(document, "div", "maestro-console-header")?;
        let label = element(document, "span", "maestro-console-label")?;
        append(&header, &label)?;
        let scroll: HtmlElement = element(document, "div", "console-scroll")?.unchecked_into();
        let pre = element(document, "pre", "")?;
        append(&scroll, &pre)?;
        Ok(Self {
            root,
            header,
            label,
            scroll,
            pre,
        })
    }

    /// Place the assembled header and scrolling body in the root.
    pub(super) fn place(&self) -> Result<(), AttachmentHostError> {
        for child in [&self.header, &self.scroll] {
            append(&self.root, child)?;
        }
        Ok(())
    }

    /// Reconcile the localized label, literal output and its semantic color.
    pub(super) fn update(&self, (label, content, variant): (&str, &str, ConsoleVariant)) {
        set_text_if_changed(&self.label, label);
        set_text_if_changed(&self.pre, content);
        self.pre.set_class_name(match variant {
            ConsoleVariant::Error => "maestro-console-error",
            ConsoleVariant::Default => "",
        });
    }

    /// Bring the console's own scroller to its bottom.
    pub(super) fn scroll_bottom(&self) {
        scroll_bottom(&self.scroll);
    }
}

/// Bring `scroll` to its bottom.
pub(crate) fn scroll_bottom(scroll: &Element) {
    scroll.set_scroll_top(scroll.scroll_height());
}

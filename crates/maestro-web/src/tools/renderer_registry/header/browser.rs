//! Retained native captions and subsection labels.
use crate::{
    browser::attachment_content::{
        self, AttachmentHost, AttachmentUiError, ReadingContent,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    tools::types::ToolRenderView,
};
use std::rc::Rc;

/// Typography and selector of a tool's literal text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextKind {
    /// Shared header caption.
    Caption,
    /// Input or output label.
    Label,
}
/// One native text lifetime beneath the caller's row state.
pub(crate) struct NativeText {
    /// Platform registry owning the published root.
    host: Rc<dyn AttachmentHost>,
    /// Stable feature identity.
    id: egui::Id,
    /// Last successfully published text and typography.
    shown: Option<(String, TextKind)>,
    /// Published factory identity.
    content: Option<Rc<ReadingContent>>,
    /// Native caption target, captured weakly by factories.
    #[cfg(target_arch = "wasm32")]
    node: Rc<std::cell::RefCell<Option<web_sys::Element>>>,
}
impl NativeText {
    /// Bind a literal text owner to the supplied platform.
    pub(crate) fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            shown: None,
            content: None,
            #[cfg(target_arch = "wasm32")]
            node: Rc::default(),
        }
    }
    /// Apply interaction color only to this literal caption, never rich descendants.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_hovered(&self, hovered: bool) -> Result<(), AttachmentUiError> {
        if let Some(node) = self.node.borrow().as_ref() {
            use wasm_bindgen::JsCast as _;
            node.unchecked_ref::<web_sys::HtmlElement>()
                .style()
                .set_property(
                    "color",
                    if hovered {
                        "var(--maestro-color-text)"
                    } else {
                        "var(--maestro-color-muted)"
                    },
                )
                .map_err(|error| attachment_content::browser::host_error(&error))?;
        }
        Ok(())
    }
    /// Stable native identity for the containing layout scope.
    pub(crate) fn id(&self) -> egui::Id {
        self.id
    }
    /// Measure and publish literal text through the existing transaction.
    pub(crate) fn render(
        &mut self,
        ui: &mut egui::Ui,
        text: (&str, TextKind),
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, AttachmentUiError> {
        let changed = self
            .shown
            .as_ref()
            .is_none_or(|shown| shown.0 != text.0 || shown.1 != text.1);
        let candidate = changed.then(|| content(text.0, text.1, self));
        let content = candidate
            .as_ref()
            .or(self.content.as_ref())
            .ok_or_else(|| AttachmentUiError::Host("Missing tool text".into()))?;
        let layout = ControlledReading {
            height: ReadingHeight::Flow,
            host: self.host.as_ref(),
            id: self.id,
            content,
            sizes: &[],
            modal: view.modal,
            presentation: view.presentation,
        }
        .draw(ui)?;
        if let Some(content) = candidate {
            self.content = Some(content);
            self.shown = Some((text.0.into(), text.1));
        }
        Ok(layout.response)
    }
}
impl Drop for NativeText {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// Build a literal factory without retaining mounted owners or frame operands.
fn content(text: &str, kind: TextKind, owner: &NativeText) -> Rc<ReadingContent> {
    #[cfg(target_arch = "wasm32")]
    {
        let text = text.to_owned();
        let target = Rc::downgrade(&owner.node);
        Rc::new(ReadingContent {
            mount: Rc::new(move |document| mount(document, &text, kind, &target)),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (text, kind, owner);
        Rc::new(ReadingContent {})
    }
}
/// Mount one text node; successful edits reconcile into the retained node.
#[cfg(target_arch = "wasm32")]
fn mount(
    document: &web_sys::Document,
    text: &str,
    kind: TextKind,
    target: &std::rc::Weak<std::cell::RefCell<Option<web_sys::Element>>>,
) -> Result<attachment_content::ReadingRoot, AttachmentUiError> {
    use attachment_content::browser::{append, element, host_error, set_text_if_changed};
    let root = element(document, "div", "maestro-tool-text")?;
    let (class, metrics) = match kind {
        TextKind::Caption => (
            "maestro-tool-caption",
            "font-size:0.875rem;line-height:1.25rem;",
        ),
        TextKind::Label => (
            "maestro-tool-label",
            "font-size:0.75rem;line-height:1rem;font-weight:500;",
        ),
    };
    let node = element(document, "span", class)?;
    node.set_attribute("style", &format!("display:block;color:var(--maestro-color-muted);font-family:var(--maestro-type-body-font);{metrics}" )).map_err(|error| host_error(&error))?;
    node.set_text_content(Some(text));
    append(&root, &node)?;
    let text = text.to_owned();
    if let Some(target) = target.upgrade() {
        *target.borrow_mut() = Some(node.clone());
    }
    let target = target.clone();
    Ok(attachment_content::ReadingRoot {
        element: root,
        slots: vec![],
        owner: Some(Box::new(node)),
        commit: Some(Box::new(move |previous| {
            if let Some(node) = previous
                .owner
                .as_ref()
                .and_then(|owner| owner.downcast_ref::<web_sys::Element>())
            {
                set_text_if_changed(node, &text);
                if let Some(target) = target.upgrade() {
                    *target.borrow_mut() = Some(node.clone());
                }
            }
        })),
    })
}

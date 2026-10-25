//! Page-mounted card bodies share the existing branch preparation and native child owners.
use super::{Body, CardContent, CardOutput};
use crate::{
    browser::attachment_content::browser::element,
    dom::{DomContent, DomRoot, DomView, MountedContent},
    tools::{
        renderer_registry::{ToolHeaderText, header::dom::render_header},
        types::ToolUiError,
    },
};
use std::cell::RefCell;
use wasm_bindgen::JsCast as _;
use web_sys::Element;

impl CardOutput for Box<dyn DomContent> {
    fn card(card: CardContent) -> Self {
        Box::new(card)
    }
}
/// Independent header and selected body lifetimes.
#[derive(Default)]
struct MountedCard {
    /// Stable header placement.
    header: MountedContent,
    /// Body placement retained only while its branch stays selected.
    body: MountedContent,
}
impl DomContent for CardContent {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-tool-card")?.into(),
            owner: Some(Box::new(RefCell::new(MountedCard::default()))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let owner = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<MountedCard>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing card state".into()))?;
        let glyph = crate::icons::icon_node(self.glyph)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?
            .ok_or_else(|| ToolUiError::Presentation(format!("Missing icon {}", self.glyph)))?;
        let header = render_header(
            self.state,
            glyph,
            ToolHeaderText::Text(self.caption.resolve(&view.i18n)?),
        );
        let body: Option<Box<dyn DomContent>> = match &self.body {
            Body::Empty => None,
            Body::Console(content, variant) => {
                let mut console = crate::ConsoleBlock::default();
                console.content.clone_from(content);
                console.variant = *variant;
                Some(Box::new(console))
            }
            Body::Error(text) => Some(Box::new(ErrorText(text.clone()))),
        };
        let mut mounted = owner.take();
        let result = (|| {
            mounted.header.show(header.as_ref(), &root.node, view)?;
            if let Some(body) = body {
                mounted.body.show(body.as_ref(), &root.node, view)?;
            } else {
                mounted.clear_body();
            }
            Ok(())
        })();
        let old = owner.replace(mounted);
        drop(old);
        result
    }
}
/// Literal destructive output selected by calculation and clock recipes.
struct ErrorText(String);
impl DomContent for ErrorText {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot::new(element(&view.document, "div", "maestro-tool-error")?);
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        root.node.set_text_content(Some(&self.0));
        Ok(())
    }
}

impl MountedCard {
    /// Release the deselected child and its native subscriptions.
    fn clear_body(&mut self) {
        if let Some(node) = self.body.root() {
            node.unchecked_ref::<Element>().remove();
        }
        self.body = MountedContent::default();
    }
}

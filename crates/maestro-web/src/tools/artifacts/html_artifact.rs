//! HTML artifact keeping its original source beside an isolated preview region.
use crate::{
    SandboxIframe,
    browser::{
        attachment_content::browser::{append, element},
        sandbox_value::BrowserSandboxValue,
    },
    dom::{DomContent, DomRoot, DomView},
    tools::{artifacts::text_artifact::source_nodes, types::ToolUiError},
};
use maestro_app::{SandboxRuntimeProvider, SandboxUrlProvider, SharedHandle};
use std::{cell::RefCell, rc::Rc};
use web_sys::Element;

/// Original HTML whose mounted bodies keep a preview region and a highlighted source listing.
#[derive(Default)]
pub struct HtmlArtifact {
    /// Authored filename, which also names the artifact's sandbox.
    pub filename: String,
    /// Providers whose globals and runtime each load receives, in order.
    pub runtime_providers: Vec<SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>>,
    /// Sandbox page address given to the frame before each load, when present.
    pub sandbox_url_provider: Option<SandboxUrlProvider>,
    /// The connected body's isolated frame, empty while no body is connected.
    pub sandbox_iframe_ref: Rc<RefCell<Option<Rc<SandboxIframe>>>>,
    /// Original source.
    content: String,
}
impl HtmlArtifact {
    /// Start with empty filename, source and providers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Borrow the original source.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
    /// Replace the original source.
    /// # Errors
    /// A detached artifact never fails.
    pub fn set_content(&mut self, content: String) -> Result<(), ToolUiError> {
        self.content = content;
        Ok(())
    }
}
/// Source listing of one mounted body and the source it shows.
struct Mounted {
    /// Highlighted listing node, kept across edits.
    code: Element,
    /// Source last written into the listing.
    shown: RefCell<String>,
}
impl DomContent for HtmlArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let node = element(document, "div", "maestro-html-artifact")?;
        let preview = element(document, "div", "maestro-html-preview")?;
        let source = element(document, "div", "maestro-html-source")?;
        source
            .set_attribute("hidden", "")
            .map_err(|error| ToolUiError::native(&error))?;
        let (listing, code) = source_nodes(document)?;
        code.set_class_name("hljs language-html");
        append(&source, &listing)?;
        append(&node, &preview)?;
        append(&node, &source)?;
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(Mounted {
                code,
                shown: RefCell::default(),
            })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("HTML body is not mounted".to_owned()))?;
        if *mounted.shown.borrow() == self.content {
            return Ok(());
        }
        match maestro_theme::highlight_html(&*view.highlighter, &self.content, Some("html")) {
            Ok(Some(html)) => mounted.code.set_inner_html(&html),
            _ => mounted.code.set_text_content(Some(&self.content)),
        }
        mounted.shown.borrow_mut().clone_from(&self.content);
        Ok(())
    }
}

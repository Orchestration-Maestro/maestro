//! Page-mounted card bodies share the existing branch preparation and native child owners.
use super::{Body, Caption, CardContent};
use crate::{
    ConsoleBlock,
    browser::attachment_content::browser::element,
    components::console_block::ConsoleVariant,
    dom::{DomContent, DomRoot, DomView, MountedContent},
    tools::{
        renderer_registry::{
            ToolHeaderRefs, ToolHeaderState, ToolHeaderText, header::dom::glyph,
            render_collapsible_header, render_header,
        },
        types::ToolUiError,
    },
};
use std::cell::RefCell;

/// Prepare a card's status header from its glyph and caption, with an artifact's filename pill
/// after its label, toggling `refs` when a body exists.
/// # Errors
/// Returns a missing glyph or localization failure.
pub(in crate::tools::renderers) fn header(
    state: ToolHeaderState,
    tool_icon: &str,
    caption: &Caption,
    view: &DomView,
    refs: Option<ToolHeaderRefs>,
) -> Result<Box<dyn DomContent>, ToolUiError> {
    let tool_icon = glyph(tool_icon)?;
    let label = caption.resolve(&view.i18n)?;
    let text = match caption {
        Caption::Artifact(_, Some(pill)) => ToolHeaderText::Content(
            crate::tools::artifacts::artifact_pill::dom::caption(label, pill),
        ),
        _ => ToolHeaderText::Text(label),
    };
    Ok(match refs {
        Some(refs) => render_collapsible_header(state, tool_icon, text, refs, false),
        None => render_header(state, tool_icon, text),
    })
}
/// Literal console output in its semantic color.
pub(in crate::tools::renderers) fn console(
    content: String,
    variant: ConsoleVariant,
) -> ConsoleBlock {
    let mut console = ConsoleBlock::default();
    console.content = content;
    console.variant = variant;
    console
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
        let header = header(self.state, self.glyph, &self.caption, view, None)?;
        let body: Option<Box<dyn DomContent>> = match &self.body {
            Body::Empty => None,
            Body::Console(content, variant) => Some(Box::new(console(content.clone(), *variant))),
            Body::Error(text) => Some(Box::new(ErrorText(text.clone()))),
        };
        let mut mounted = owner.take();
        let result = (|| {
            mounted.header.show(header.as_ref(), &root.node, view)?;
            if let Some(body) = body {
                mounted.body.show(body.as_ref(), &root.node, view)?;
            } else {
                mounted.body.clear();
            }
            Ok(())
        })();
        let old = owner.replace(mounted);
        drop(old);
        result
    }
}
/// Literal destructive output selected by calculation, clock and artifact recipes.
pub(in crate::tools::renderers) struct ErrorText(pub(in crate::tools::renderers) String);
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

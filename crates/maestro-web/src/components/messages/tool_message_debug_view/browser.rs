//! Native mounting of the call and result listings.
use super::ToolMessageDebugView;
use crate::{
    browser::attachment_content::browser::{append, element, set_text_if_changed},
    components::messages::tool_message_debug_view::pretty,
    dom::{self, DomContent, DomRoot, DomView, MountedContent},
    tools::{renderers::default_renderer::Listing, types::ToolUiError},
};
use std::cell::RefCell;
use web_sys::Element;

/// Mounted labels, listings and the absent-result text.
struct Mounted {
    /// Call heading.
    call_label: Element,
    /// Result heading.
    result_label: Element,
    /// Result section receiving listings or the absent text.
    result: Element,
    /// Always visible call listing.
    call: MountedContent,
    /// Output and details listings, mounted only while a result is shown.
    listings: [MountedContent; 2],
    /// Localized absent-result text, present only without a result view.
    absent: Option<Element>,
}
impl DomContent for ToolMessageDebugView {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let document = &view.document;
        let root = element(document, "div", "maestro-tool-debug")?;
        let call = element(document, "div", "")?;
        let result = element(document, "div", "")?;
        let call_label = element(document, "div", "maestro-tool-section-label")?;
        let result_label = element(document, "div", "maestro-tool-section-label")?;
        append(&call, &call_label)?;
        append(&result, &result_label)?;
        append(&root, &call)?;
        append(&root, &result)?;
        let root = DomRoot {
            node: root.into(),
            owner: Some(Box::new(RefCell::new(Mounted {
                call_label,
                result_label,
                result,
                call: MountedContent::default(),
                listings: Default::default(),
                absent: None,
            }))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Prepare every listing before changing any mounted text.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<Mounted>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing debug view state".into()))?;
        let labels = [view.i18n.text("Call")?, view.i18n.text("Result")?];
        // The call listing is always labelled JSON, even for literal text.
        let call = listing((pretty(self.call_args.as_ref())?.0, "json"));
        let shown = if self.has_result {
            Ok(self.result_listings()?)
        } else {
            Err(view.i18n.text("(no result)")?)
        };
        let call_section = root
            .node
            .first_child()
            .ok_or_else(|| ToolUiError::Presentation("Missing debug call section".into()))?;
        let mut mounted = mounted.borrow_mut();
        set_text_if_changed(&mounted.call_label, &labels[0]);
        set_text_if_changed(&mounted.result_label, &labels[1]);
        mounted.call.show(&call, &call_section, view)?;
        match shown {
            Ok(listings) => mounted.show_listings(&listings, view),
            Err(absent) => mounted.show_absent(&absent, view),
        }
    }
}
impl ToolMessageDebugView {
    /// The joined text output and the details, each strictly parsed or kept literal.
    fn result_listings(&self) -> Result<[Listing; 2], ToolUiError> {
        let output = self.result.as_ref().map_or_else(String::new, |result| {
            maestro_app::UserBlock::text_output(&result.content)
        });
        let details = self
            .result
            .as_ref()
            .and_then(|result| result.details.as_ref());
        Ok([
            listing(pretty(Some(&serde_json::Value::String(output)))?),
            listing(pretty(details)?),
        ])
    }
}
impl Mounted {
    /// Replace the absent text with the two result listings, updating retained ones in place.
    fn show_listings(
        &mut self,
        listings: &[Listing; 2],
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        if let Some(absent) = self.absent.take() {
            absent.remove();
        }
        for (slot, listing) in self.listings.iter_mut().zip(listings) {
            slot.show(listing, &self.result, view)?;
        }
        Ok(())
    }
    /// Replace the result listings with the localized absent text.
    fn show_absent(&mut self, text: &str, view: &DomView) -> Result<(), ToolUiError> {
        for slot in &mut self.listings {
            slot.clear();
        }
        if self.absent.is_none() {
            let absent = element(&view.document, "div", "maestro-tool-detail")?;
            append(&self.result, &absent)?;
            self.absent = Some(absent);
        }
        if let Some(absent) = &self.absent {
            set_text_if_changed(absent, text);
        }
        Ok(())
    }
}
/// A literal listing that is never decoded as base64 input.
fn listing((code, language): (String, &'static str)) -> Listing {
    Listing { code, language }
}

//! Literal debug listings with strict JSON-string projection.
use super::reading_view;
use crate::{
    browser::attachment_content::AttachmentHost,
    components::{
        code_block::CodeBlock,
        native_text::{NativeText, TextKind},
    },
    tools::types::{ToolRenderView, ToolUiError},
};
use std::rc::Rc;

/// Retained call and result listings, independent of result-payload presence.
pub struct ToolMessageDebugView {
    /// Original optional call arguments; absence displays empty literal code.
    pub call_args: Option<serde_json::Value>,
    /// Optional result payload, independent of the visibility flag.
    pub result: Option<maestro_app::ToolResultMessage>,
    /// Whether output and details listings are visible.
    pub has_result: bool,
    /// Platform effects for newly visible listings.
    host: Rc<dyn AttachmentHost>,
    /// Call heading.
    call_label: NativeText,
    /// Result heading.
    result_label: NativeText,
    /// Always visible call listing.
    call: CodeBlock,
    /// Output and details lifetimes, present only with a result view.
    listings: Option<(CodeBlock, CodeBlock)>,
    /// Localized absent-result lifetime.
    absent: Option<NativeText>,
}
impl ToolMessageDebugView {
    /// Bind empty debug inputs to the supplied platform.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            call_args: None,
            result: None,
            has_result: false,
            call_label: NativeText::new(Rc::clone(&host)),
            result_label: NativeText::new(Rc::clone(&host)),
            call: CodeBlock::new_literal(Rc::clone(&host)),
            listings: None,
            absent: None,
            host,
        }
    }
    /// Draw localized labels and literal listings through the existing code owner.
    /// # Errors
    /// Returns reached localization, formatting, highlighting or native publication failures.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        let call = pretty(self.call_args.as_ref())?;
        let row = ui.vertical(|ui| -> Result<(), ToolUiError> {
            ui.add_space(12.);
            ui.spacing_mut().item_spacing.y = 4.;
            self.call_label
                .render(ui, (&view.i18n.text("Call")?, TextKind::Label), view)?;
            listing(&mut self.call, (call.0, "json"), ui, view)?;
            ui.add_space(4.);
            self.result_label
                .render(ui, (&view.i18n.text("Result")?, TextKind::Label), view)?;
            if self.has_result {
                self.result_listings(ui, view)?;
            } else {
                self.listings = None;
                self.absent
                    .get_or_insert_with(|| NativeText::new(Rc::clone(&self.host)))
                    .render(ui, (&view.i18n.text("(no result)")?, TextKind::Label), view)?;
            }
            Ok(())
        });
        row.inner?;
        Ok(row.response)
    }
    /// Prepare and publish the two independently retained result listings.
    fn result_listings(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<(), ToolUiError> {
        self.absent = None;
        let output = self.result.as_ref().map_or_else(String::new, |result| {
            maestro_app::UserBlock::text_output(&result.content)
        });
        let output = pretty(Some(&serde_json::Value::String(output)))?;
        let details = pretty(
            self.result
                .as_ref()
                .and_then(|result| result.details.as_ref()),
        )?;
        let (output_block, details_block) = self.listings.get_or_insert_with(|| {
            (
                CodeBlock::new_literal(Rc::clone(&self.host)),
                CodeBlock::new_literal(Rc::clone(&self.host)),
            )
        });
        listing(output_block, output, ui, view)?;
        listing(details_block, details, ui, view)?;
        Ok(())
    }
}
/// Preserve missing values, strict parsed strings and raw literal fallbacks.
fn pretty(value: Option<&serde_json::Value>) -> Result<(String, &'static str), ToolUiError> {
    let Some(value) = value else {
        return Ok((String::new(), "json"));
    };
    let parsed;
    let value = if let Some(text) = value.as_str() {
        match maestro_app::browser::utils::parse_json(text) {
            Ok(value) => {
                parsed = value;
                &parsed
            }
            Err(_) => return Ok((text.into(), "text")),
        }
    } else {
        value
    };
    maestro_app::pretty_json(value)
        .map(|text| (text, "json"))
        .map_err(|error| ToolUiError::Presentation(error.to_string()))
}
/// Draw already literal text, never the public base64-input path.
fn listing(
    block: &mut CodeBlock,
    content: (String, &str),
    ui: &mut egui::Ui,
    view: ToolRenderView<'_>,
) -> Result<(), ToolUiError> {
    block.code = content.0;
    content.1.clone_into(&mut block.language);
    block
        .render(ui, reading_view(view))
        .map_err(ToolUiError::reading)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::pretty;
    use serde_json::{Value, json};

    #[test]
    fn debug_preparation_keeps_missing_strict_and_literal_values() {
        assert_eq!(pretty(None).unwrap(), (String::new(), "json"));
        for (value, content, language) in [
            (Value::Null, "null", "json"),
            (json!("YWJj"), "YWJj", "text"),
            (json!("1e309"), "null", "json"),
            (json!("{\"n\":1e309}"), "{\n  \"n\": null\n}", "json"),
            (json!("\u{feff}null"), "\u{feff}null", "text"),
            (json!("\"raw\ncontrol\""), "\"raw\ncontrol\"", "text"),
        ] {
            assert_eq!(pretty(Some(&value)).unwrap(), (content.into(), language));
        }
    }
}

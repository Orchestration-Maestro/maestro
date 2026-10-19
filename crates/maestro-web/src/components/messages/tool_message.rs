//! Call-scoped dispatch through a retained tool row.
use super::color;
use crate::tools::types::{
    ToolContentOutput, ToolParams, ToolRenderState, ToolRenderView, ToolUiError,
};
use maestro_app::{ToolCall, ToolResultMessage};

/// Borrow one tool invocation without narrowing its native result details.
pub struct ToolMessageProps<'a, D = serde_json::Value> {
    /// Original model tool call.
    pub tool_call: &'a ToolCall,
    /// Optional registered tool name.
    pub tool: Option<&'a str>,
    /// Original result envelope.
    pub result: Option<&'a ToolResultMessage<D>>,
    /// Whether execution is pending.
    pub pending: bool,
    /// Replace any result with a synthetic abort.
    pub aborted: bool,
    /// Whether arguments are still streaming.
    pub is_streaming: bool,
}
impl<D> Copy for ToolMessageProps<'_, D> {}
impl<D> Clone for ToolMessageProps<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}
/// One tool row and its selected renderer's widget lifetime.
#[derive(Default)]
pub struct ToolMessage {
    /// Retained recipe state and native placement scope.
    retained: ToolRenderState,
}
impl ToolMessage {
    /// Create an empty retained tool row.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Dispatch borrowed operands once, wrapping only default content in a card.
    /// # Errors
    /// Returns reached dispatch, clock, reading or presentation failures.
    pub fn render<D: 'static>(
        &mut self,
        ui: &mut egui::Ui,
        props: ToolMessageProps<'_, D>,
        view: ToolRenderView<'_>,
    ) -> ToolContentOutput {
        let mut overlays = Vec::new();
        let response = self.draw(ui, props, view, &mut overlays);
        ToolContentOutput { response, overlays }
    }
    /// Draw the row while transferring opened owners before any later error.
    fn draw<D: 'static>(
        &mut self,
        ui: &mut egui::Ui,
        props: ToolMessageProps<'_, D>,
        view: ToolRenderView<'_>,
        overlays: &mut Vec<crate::AttachmentOverlay>,
    ) -> Result<egui::Response, ToolUiError> {
        let aborted = if props.aborted {
            #[cfg(target_arch = "wasm32")]
            let timestamp = js_sys::Date::now();
            #[cfg(not(target_arch = "wasm32"))]
            let timestamp = epoch_milliseconds()?;
            Some(ToolResultMessage {
                tool_call_id: props.tool_call.id.clone(),
                tool_name: props.tool_call.name.clone(),
                content: Vec::new(),
                details: None,
                is_error: true,
                timestamp,
            })
        } else {
            None
        };
        let result = aborted.as_ref().or(props.result);
        let name = props
            .tool
            .filter(|name| !name.is_empty())
            .unwrap_or(&props.tool_call.name);
        let rendered = crate::render_tool(
            name,
            Some(ToolParams::object(&props.tool_call.arguments)),
            result,
            !props.aborted && (props.is_streaming || props.pending),
        )?;
        if rendered.is_custom {
            return rendered
                .render(ui, view, &mut self.retained)
                .collect(overlays);
        }
        let frame = egui::Frame::new()
            .fill(color(view, "panel")?)
            .corner_radius(6.)
            .shadow(egui::epaint::Shadow {
                offset: [0, 1],
                blur: 2,
                spread: 0,
                color: color(view, "text")?.gamma_multiply(0.05),
            })
            .stroke(egui::Stroke::new(1., color(view, "border")?))
            .inner_margin(10);
        let row = frame.show(ui, |ui| {
            rendered
                .render(ui, view, &mut self.retained)
                .collect(overlays)
        });
        row.inner?;
        Ok(row.response)
    }
}
/// Observe whole wall-clock milliseconds on native targets when constructing an abort.
#[cfg(not(target_arch = "wasm32"))]
fn epoch_milliseconds() -> Result<f64, ToolUiError> {
    use num_traits::ToPrimitive as _;
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| ToolUiError::Presentation(error.to_string()))?
        .as_millis()
        .to_f64()
        .ok_or_else(|| ToolUiError::Presentation("Epoch milliseconds are not representable".into()))
}

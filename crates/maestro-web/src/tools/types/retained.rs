//! Call-scoped access to a row's existing retained state.
use super::{ToolContentOutput, ToolUiError};
use maestro_app::ToolRendererState;
/// Draw outside the state-slot borrow and restore it before returning either result.
pub(crate) fn render_retained<R: Default + 'static>(
    state: &ToolRendererState,
    draw: impl FnOnce(&mut R, &mut Vec<crate::AttachmentOverlay>) -> Result<egui::Response, ToolUiError>,
) -> ToolContentOutput {
    let slot = state.state::<R>();
    let mut retained = std::mem::take(&mut *slot.borrow_mut());
    let mut overlays = Vec::new();
    let response = draw(&mut retained, &mut overlays);
    drop(slot.replace(retained));
    ToolContentOutput { response, overlays }
}
/// Implement the common retained drawing route for a typed recipe.
macro_rules! retained_tool_content {
    ($content:ty, $retained:ty) => {
        impl crate::tools::types::ToolContent for $content {
            fn render(
                &self,
                ui: &mut egui::Ui,
                view: crate::tools::types::ToolRenderView<'_>,
                state: &maestro_app::ToolRendererState,
            ) -> crate::tools::types::ToolContentOutput {
                crate::tools::types::render_retained::<$retained>(state, |retained, overlays| {
                    self.draw(ui, view, retained, overlays)
                })
            }
        }
    };
}
pub(crate) use retained_tool_content;

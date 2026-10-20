//! Retained literal artifact filenames with independently consuming panel actions.
use crate::{
    components::native_text::{NativeText, TextKind},
    tools::{
        renderer_registry::header::InlineActions,
        types::{ToolContent, ToolContentOutput, ToolRenderView},
    },
};
use maestro_app::{ToolRendererState, browser::tools::artifacts::LocalArtifactsPanel};
use std::rc::Rc;

/// Owned display and identity; malformed display values have no artifact identity.
#[derive(Clone)]
pub(in crate::tools) struct Pill {
    /// Literal display-only text.
    pub(in crate::tools) text: String,
    /// Authored string identity, never derived from formatted display text.
    pub(in crate::tools) filename: Option<String>,
    /// Panel selected when the recipe was prepared.
    pub(in crate::tools) panel: Option<Rc<LocalArtifactsPanel>>,
    /// Containing header's activation/exclusion owner.
    pub(in crate::tools) actions: Option<Rc<InlineActions>>,
}
/// Prepare a selectable literal filename, capturing the supplied panel handle.
#[must_use]
pub fn artifact_pill(
    filename: String,
    artifacts_panel: Option<Rc<LocalArtifactsPanel>>,
) -> Box<dyn ToolContent> {
    Box::new(Pill {
        text: filename.clone(),
        filename: Some(filename),
        panel: artifacts_panel,
        actions: None,
    })
}
/// One mounted filename and its primary-pointer transaction.
#[derive(Default)]
struct Retained {
    /// Native text lifetime.
    text: Option<NativeText>,
    /// Admitted primary press, owned by the shared pointer helper.
    pressed: Option<()>,
}
impl ToolContent for Pill {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &ToolRendererState,
    ) -> ToolContentOutput {
        ToolContentOutput::retained(state, |retained: &mut Retained, _| {
            self.draw(ui, view, retained)
        })
    }
}
impl Pill {
    /// Draw the selected native filename through clipped pointer admission.
    fn draw(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        retained: &mut Retained,
    ) -> Result<egui::Response, crate::tools::types::ToolUiError> {
        let text = retained
            .text
            .get_or_insert_with(|| NativeText::new(Rc::clone(view.host)));
        let response = text.render(ui, (&self.text, TextKind::Pill(self.panel.is_some())), view)?;
        let visible = response.rect.intersect(ui.clip_rect());
        if self.panel.is_some()
            && let Some(actions) = &self.actions
        {
            actions.excluded.set(Some(response.rect));
        }
        let slots = if response.enabled() && visible.is_positive() {
            vec![((), visible, response.layer_id)]
        } else {
            vec![]
        };
        let action = self.action(response.enabled() && visible.is_positive());
        let events = ui.input(|input| input.events.clone());
        if self.panel.is_some()
            && !crate::browser::attachment_content::reading::controls::pointer_actions(
                &events,
                &mut retained.pressed,
                &slots,
                ui.ctx(),
            )
            .is_empty()
        {
            action();
        }
        text.set_action(Some(action));
        Ok(response)
    }
    /// Native callbacks never capture their mounted text owner.
    fn action(&self, admitted: bool) -> Rc<dyn Fn()> {
        if let Some(panel) = &self.panel {
            return panel_action(Rc::clone(panel), self.filename.clone(), admitted);
        }
        let action = self.actions.as_ref().map(InlineActions::action);
        Rc::new(move || {
            if admitted && let Some(action) = &action {
                action();
            }
        })
    }
}

/// Capture a panel action independently of its mounted native owner.
fn panel_action(
    panel: Rc<LocalArtifactsPanel>,
    filename: Option<String>,
    admitted: bool,
) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if admitted && let Some(filename) = &filename {
            panel.open_artifact(filename);
        }
    })
}

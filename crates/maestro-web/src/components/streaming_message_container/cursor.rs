//! Native cursor decoration beneath the assistant content.
use crate::{
    browser::attachment_content::{self, AttachmentHost, ReadingContent},
    tools::types::{ToolRenderView, ToolUiError},
};
use std::rc::Rc;

/// One retained native cursor with brand-selected paint.
pub(super) struct Cursor {
    /// Existing native placement owner.
    host: Rc<dyn AttachmentHost>,
    /// Stable cursor surface identity.
    id: egui::Id,
    /// Last selected muted color and its native factory.
    content: Option<(String, Rc<ReadingContent>)>,
}
impl Cursor {
    /// Create an unmounted cursor owner.
    pub(super) fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            content: None,
        }
    }
    /// Allocate the source dimensions and publish the actual native animation.
    pub(super) fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<(), ToolUiError> {
        let color = view.presentation.colors["muted"];
        if self
            .content
            .as_ref()
            .is_none_or(|(shown, _)| shown != color)
        {
            self.content = Some((color.to_owned(), factory(color)));
        }
        if let Some((_, content)) = &self.content {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.;
                ui.add_space(16.);
                let (_, rect) = ui.allocate_space(egui::vec2(8., 16.));
                attachment_content::reading::present(
                    self.host.as_ref(),
                    ui,
                    (self.id, rect, view.modal),
                    content,
                    view.presentation,
                )
            })
            .inner?;
        }
        Ok(())
    }
}
impl Drop for Cursor {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// Keep animation in native CSS rather than recreating a frame scheduler.
fn factory(color: &str) -> Rc<ReadingContent> {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = color;
    Rc::new(ReadingContent {
        #[cfg(target_arch = "wasm32")]
        mount: {
            let color = color.to_owned();
            Rc::new(move |document| {
                use attachment_content::browser::{append, element, host_error};
                let root = element(document, "span", "maestro-transcript-cursor")?;
                root.set_attribute("style", &format!("--transcript-muted:{color};"))
                    .map_err(|error| host_error(&error))?;
                let style = element(document, "style", "")?;
                style.set_text_content(Some(include_str!("../transcript.css")));
                append(&root, &style)?;
                Ok(attachment_content::ReadingRoot {
                    frame_style: "",
                    decoration: None,
                    element: root,
                    slots: vec![],
                    owner: None,
                    commit: None,
                })
            })
        },
    })
}

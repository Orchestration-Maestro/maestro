//! Selectable line differences using the application's borrowed change runs.
use crate::{
    browser::attachment_content::{
        self, AttachmentHost, ReadingContent,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    tools::types::{ToolContent, ToolContentOutput, ToolRenderView},
};
use maestro_app::ToolRendererState;
use std::rc::Rc;

/// Literal operands for a line difference view.
#[derive(PartialEq, Eq)]
pub struct DiffProps {
    /// Original text, including its line endings.
    pub old_text: String,
    /// Replacement text, including its line endings.
    pub new_text: String,
    /// Optional literal header; an empty title omits the header.
    pub title: Option<String>,
    /// Caller-supplied native CSS classes.
    pub class_name: String,
}
/// Browser row preparation and its native mount.
#[cfg(target_arch = "wasm32")]
mod browser;
/// Browser cache equality uses the selected rendered lines.
#[cfg(target_arch = "wasm32")]
use browser::Lines as Prepared;
/// Native cache equality needs only the authored operands.
#[cfg(not(target_arch = "wasm32"))]
type Prepared = DiffProps;

/// Prepare a bordered native line difference with a locally scrolling body.
#[must_use]
pub fn diff(props: DiffProps) -> Box<dyn ToolContent> {
    #[cfg(target_arch = "wasm32")]
    let props = browser::prepare(props);
    Box::new(DiffContent(Rc::new(props)))
}
/// Immutable recipe; native roots belong to each rendered row.
struct DiffContent(Rc<Prepared>);
/// Native publication lifetime for a single diff.
struct Retained {
    /// Platform which owns the mounted surface.
    host: Rc<dyn AttachmentHost>,
    /// Stable surface identity.
    id: egui::Id,
    /// Last successfully published operands and factory.
    shown: Option<(Rc<Prepared>, Rc<ReadingContent>)>,
}
impl Drop for Retained {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
impl ToolContent for DiffContent {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &ToolRendererState,
    ) -> ToolContentOutput {
        let slot = state.state::<Option<Retained>>();
        let mut retained = slot.borrow_mut().take().unwrap_or_else(|| Retained {
            host: Rc::clone(view.host),
            id: attachment_content::identity(),
            shown: None,
        });
        let changed = retained
            .shown
            .as_ref()
            .is_none_or(|(lines, _)| **lines != *self.0);
        let content = retained
            .shown
            .as_ref()
            .filter(|_| !changed)
            .map_or_else(|| content(&self.0), |(_, content)| Rc::clone(content));
        let response = ControlledReading {
            intrinsic_width: false,
            height: ReadingHeight::Flow,
            host: retained.host.as_ref(),
            id: retained.id,
            content: &content,
            sizes: &[],
            modal: view.modal,
            presentation: view.presentation,
        }
        .draw(ui)
        .map(|layout| layout.response)
        .map_err(Into::into);
        if response.is_ok() {
            retained.shown = Some((Rc::clone(&self.0), content));
        }
        drop(slot.replace(Some(retained)));
        ToolContentOutput {
            response,
            overlays: Vec::new(),
        }
    }
}
/// Build an owned literal native factory without retaining a mounted owner.
fn content(lines: &Rc<Prepared>) -> Rc<ReadingContent> {
    #[cfg(target_arch = "wasm32")]
    {
        let lines = Rc::clone(lines);
        Rc::new(ReadingContent {
            mount: Rc::new(move |document| browser::mount(document, &lines)),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = lines;
        Rc::new(ReadingContent {})
    }
}

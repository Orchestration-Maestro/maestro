//! Selectable line differences using the application's borrowed change runs.
use crate::{
    browser::attachment_content::{
        self, AttachmentHost, ReadingContent,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    tools::types::{ToolContent, ToolContentOutput, ToolRenderView},
};
use maestro_app::{Change, ToolRendererState, diff_lines};
use std::rc::Rc;

/// Literal operands for a line difference view.
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
/// Browser count projection and retained native nodes.
#[cfg(target_arch = "wasm32")]
mod browser;

/// Selected line content used by retained equality and browser projection.
#[derive(PartialEq, Eq)]
struct Row {
    /// Change sign, or a space for context.
    sign: char,
    /// Literal text after the sign and separator.
    text: String,
}
/// One semantic input shared by the retained key and browser presentation.
#[derive(PartialEq, Eq)]
struct Normalized {
    /// Optional nonempty literal title.
    title: Option<String>,
    /// Caller-supplied class names, preserved literally.
    class_name: String,
    /// Ordered selected lines with repetition preserved.
    rows: Vec<Row>,
}
/// Select lines after differencing so line endings still determine matching order.
fn normalize(props: DiffProps) -> Normalized {
    let mut rows = Vec::new();
    for change in diff_lines(&props.old_text, &props.new_text) {
        let (sign, text) = match change {
            Change::Unchanged(text) => (' ', text),
            Change::Added(text) => ('+', text),
            Change::Removed(text) => ('-', text),
        };
        rows.extend(text.split_terminator('\n').map(|text| Row {
            sign,
            text: text.into(),
        }));
    }
    Normalized {
        title: props.title.filter(|title| !title.is_empty()),
        class_name: props.class_name,
        rows,
    }
}

/// Prepare a bordered native line difference with a locally scrolling body.
#[must_use]
pub fn diff(props: DiffProps) -> Box<dyn ToolContent> {
    Box::new(DiffContent(Rc::new(normalize(props))))
}
/// Immutable recipe; native roots belong to each rendered row.
struct DiffContent(Rc<Normalized>);
/// Native publication lifetime for a single diff.
struct Retained {
    /// Platform which owns the mounted surface.
    host: Rc<dyn AttachmentHost>,
    /// Stable surface identity.
    id: egui::Id,
    /// Last successfully published operands and factory.
    shown: Option<(Rc<Normalized>, Rc<ReadingContent>)>,
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
fn content(lines: &Rc<Normalized>) -> Rc<ReadingContent> {
    #[cfg(target_arch = "wasm32")]
    {
        let lines = Rc::new(browser::prepare(Rc::clone(lines)));
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

//! Selectable line differences using the application's borrowed change runs.
use crate::dom::PreparedContent;
use maestro_app::{Change, diff_lines};
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
pub fn diff(props: DiffProps) -> PreparedContent {
    Box::new(DiffContent(Rc::new(normalize(props))))
}
/// Immutable recipe; native roots belong to each mounted root.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native targets prepare records only the browser mounts"
    )
)]
struct DiffContent(Rc<Normalized>);

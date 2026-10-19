//! Selectable line differences using the application's borrowed change runs.
use crate::{
    browser::attachment_content::{
        self, AttachmentHost, ReadingContent,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    tools::types::{ToolContent, ToolContentOutput, ToolRenderView},
};
use maestro_app::{Change, ToolRendererState, diff_lines};
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
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
/// One rendered line, retaining its sign and literal content.
#[derive(PartialEq, Eq)]
struct Row {
    /// Change sign, or a space for context.
    sign: char,
    /// Literal text after the sign and separator.
    text: String,
}
/// Owned selected output, independent of the original operands.
#[derive(PartialEq, Eq)]
struct Lines {
    /// Optional nonempty literal title.
    title: Option<String>,
    /// Caller-supplied class names.
    class_name: String,
    /// Ordered lines with repetition preserved.
    rows: Vec<Row>,
    /// Rendered added-line count.
    added: usize,
    /// Rendered removed-line count.
    removed: usize,
}
/// Prepare a bordered native line difference with a locally scrolling body.
#[must_use]
pub fn diff(props: DiffProps) -> Box<dyn ToolContent> {
    let mut lines = Lines {
        title: props.title.filter(|title| !title.is_empty()),
        class_name: props.class_name,
        rows: Vec::new(),
        added: 0,
        removed: 0,
    };
    for change in diff_lines(&props.old_text, &props.new_text) {
        let (sign, text) = match change {
            Change::Unchanged(text) => (' ', text),
            Change::Added(text) => ('+', text),
            Change::Removed(text) => ('-', text),
        };
        let mut rows: Vec<_> = text.split('\n').collect();
        if rows.last() == Some(&"") {
            rows.pop();
        }
        for text in rows {
            lines.added += usize::from(sign == '+');
            lines.removed += usize::from(sign == '-');
            lines.rows.push(Row {
                sign,
                text: text.into(),
            });
        }
    }
    Box::new(DiffContent(Rc::new(lines)))
}
/// Immutable recipe; native roots belong to each rendered row.
struct DiffContent(Rc<Lines>);
/// Native publication lifetime for a single diff.
struct Retained {
    /// Platform which owns the mounted surface.
    host: Rc<dyn AttachmentHost>,
    /// Stable surface identity.
    id: egui::Id,
    /// Last successfully published operands and factory.
    shown: Option<(Rc<Lines>, Rc<ReadingContent>)>,
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
fn content(lines: &Rc<Lines>) -> Rc<ReadingContent> {
    #[cfg(target_arch = "wasm32")]
    {
        let lines = Rc::clone(lines);
        Rc::new(ReadingContent {
            mount: Rc::new(move |document| mount(document, &lines)),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = lines;
        Rc::new(ReadingContent {})
    }
}
/// Mount literal rows and the optional title/count header.
#[cfg(target_arch = "wasm32")]
fn mount(
    document: &web_sys::Document,
    lines: &Rc<Lines>,
) -> Result<attachment_content::ReadingRoot, attachment_content::AttachmentHostError> {
    use attachment_content::browser::{append, element};
    let root = element(
        document,
        "div",
        &format!("maestro-diff {}", lines.class_name),
    )?;
    let style = element(document, "style", "")?;
    style.set_text_content(Some(include_str!("diff.css")));
    append(&root, &style)?;
    let mut mounted = Mounted {
        root: root.clone(),
        header: None,
        scroll: element(document, "div", "maestro-diff-scroll")?,
        title: None,
        counts: None,
        rows: Vec::new(),
    };
    mount_header(document, lines, &mut mounted)?;

    for _ in &lines.rows {
        let line = element(document, "pre", "")?;
        append(&mounted.scroll, &line)?;
        mounted.rows.push(line);
    }
    append(&root, &mounted.scroll)?;
    mounted.update(lines);
    let lines = Rc::clone(lines);
    let mounted = Rc::new(RefCell::new(mounted));
    Ok(attachment_content::ReadingRoot {
        frame_style: "",
        decoration: None,
        element: root,
        slots: vec![],
        owner: Some(Box::new(Rc::clone(&mounted))),
        commit: Some(Box::new(move |previous| {
            if let Some(previous) = previous
                .owner
                .as_ref()
                .and_then(|owner| owner.downcast_ref::<Rc<RefCell<Mounted>>>())
            {
                previous.borrow_mut().reconcile(&mounted.borrow(), &lines)?;
            }
            Ok(())
        })),
    })
}

/// Build the optional title and semantically colored count spans.
#[cfg(target_arch = "wasm32")]
fn mount_header(
    document: &web_sys::Document,
    lines: &Lines,
    mounted: &mut Mounted,
) -> Result<(), attachment_content::AttachmentHostError> {
    use attachment_content::browser::{append, element};
    if let Some(title) = &lines.title {
        let header = element(document, "div", "maestro-diff-header")?;
        let label = element(document, "span", "")?;
        label.set_text_content(Some(title));
        append(&header, &label)?;
        let counts = element(document, "span", "maestro-diff-counts")?;
        let added = element(document, "span", "maestro-diff-added-count")?;
        let separator = element(document, "span", "")?;
        separator.set_text_content(Some(" / "));
        let removed = element(document, "span", "maestro-diff-removed-count")?;
        append(&counts, &added)?;
        append(&counts, &separator)?;
        append(&counts, &removed)?;
        mounted.title = Some(label);
        mounted.counts = Some((added, removed));
        append(&header, &counts)?;
        append(&mounted.root, &header)?;
        mounted.header = Some(header);
    }
    Ok(())
}

/// Retained native nodes updated without disturbing unchanged text selection.
#[cfg(target_arch = "wasm32")]
struct Mounted {
    /// Bordered container with caller-selected classes.
    root: web_sys::Element,
    /// Locally scrolling row container.
    scroll: web_sys::Element,
    /// Optional title/count header.
    header: Option<web_sys::Element>,
    /// Optional caller-authored title.
    title: Option<web_sys::Element>,
    /// Optional added and removed counts.
    counts: Option<(web_sys::Element, web_sys::Element)>,
    /// Ordered rows, including unchanged context.
    rows: Vec<web_sys::Element>,
}
#[cfg(target_arch = "wasm32")]
impl Mounted {
    /// Admit complete row growth before changing retained values or removing rows.
    fn reconcile(
        &mut self,
        staged: &Self,
        lines: &Lines,
    ) -> Result<(), attachment_content::AttachmentHostError> {
        use attachment_content::browser::host_error;
        let document = self.root.owner_document().ok_or_else(|| {
            attachment_content::AttachmentHostError::Host("Diff has no document".into())
        })?;
        let growth = document.create_document_fragment();
        for row in staged.rows.iter().skip(self.rows.len()) {
            growth.append_child(row).map_err(|e| host_error(&e))?;
        }
        let inserted = self
            .header
            .is_none()
            .then_some(staged.header.as_ref())
            .flatten();
        if let Some(header) = inserted {
            self.root
                .insert_before(header, Some(&self.scroll))
                .map_err(|e| host_error(&e))?;
        }
        if let Err(error) = self
            .scroll
            .append_child(&growth)
            .map_err(|e| host_error(&e))
        {
            if let Some(header) = inserted {
                header.remove();
            }
            return Err(error);
        }
        self.rows
            .extend(staged.rows.iter().skip(self.rows.len()).cloned());
        for row in self.rows.drain(lines.rows.len()..) {
            row.remove();
        }
        if staged.header.is_none() {
            if let Some(header) = self.header.take() {
                header.remove();
            }
            self.title = None;
            self.counts = None;
        } else if self.header.is_none() {
            self.header.clone_from(&staged.header);
            self.title.clone_from(&staged.title);
            self.counts.clone_from(&staged.counts);
        }
        self.update(lines);
        Ok(())
    }
    /// Commit prepared values without replacing unchanged text nodes.
    fn update(&self, lines: &Lines) {
        use attachment_content::browser::set_text_if_changed;
        self.root
            .set_class_name(&format!("maestro-diff {}", lines.class_name));
        if let Some((node, title)) = self.title.as_ref().zip(lines.title.as_ref()) {
            set_text_if_changed(node, title);
        }
        if let Some((added, removed)) = &self.counts {
            set_text_if_changed(added, &format!("+{}", lines.added));
            set_text_if_changed(removed, &format!("-{}", lines.removed));
        }
        for (node, row) in self.rows.iter().zip(&lines.rows) {
            node.set_class_name(match row.sign {
                '+' => "maestro-diff-added",
                '-' => "maestro-diff-removed",
                _ => "",
            });
            set_text_if_changed(node, &format!("{} {}", row.sign, row.text));
        }
    }
}

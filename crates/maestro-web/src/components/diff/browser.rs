//! Browser-owned count projection, retained difference nodes and their page content.
use super::Normalized;
use crate::{
    browser::attachment_content,
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::{cell::RefCell, rc::Rc};

/// Browser counts derived from the same input used by retained equality.
pub(super) struct Lines {
    /// Shared normalized input, without a second line vector or cache key.
    input: Rc<Normalized>,
    /// Rendered added-line count.
    added: usize,
    /// Rendered removed-line count.
    removed: usize,
}
/// Derive browser counts once for a new native factory.
pub(super) fn prepare(input: Rc<Normalized>) -> Lines {
    let (added, removed) = input.rows.iter().fold((0, 0), |(added, removed), row| {
        (
            added + usize::from(row.sign == '+'),
            removed + usize::from(row.sign == '-'),
        )
    });
    Lines {
        input,
        added,
        removed,
    }
}
/// Build literal rows and the optional title/count header.
fn build(
    document: &web_sys::Document,
    lines: &Lines,
) -> Result<Mounted, attachment_content::AttachmentHostError> {
    use attachment_content::browser::{append, element};
    let root = element(
        document,
        "div",
        &format!("maestro-diff {}", lines.input.class_name),
    )?;
    let mut mounted = Mounted {
        root,
        header: None,
        scroll: element(document, "div", "maestro-diff-scroll")?,
        title: None,
        counts: None,
        rows: Vec::new(),
    };
    mount_header(document, lines, &mut mounted)?;
    for _ in &lines.input.rows {
        let line = element(document, "pre", "")?;
        append(&mounted.scroll, &line)?;
        mounted.rows.push(line);
    }
    append(&mounted.root, &mounted.scroll)?;
    mounted.update(lines);
    Ok(mounted)
}
/// The shown input and its retained nodes.
type Shown = RefCell<(Rc<super::Normalized>, Mounted)>;
impl DomContent for super::DiffContent {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        crate::dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let mounted = build(&view.document, &prepare(Rc::clone(&self.0)))?;
        Ok(DomRoot {
            node: mounted.root.clone().into(),
            owner: Some(Box::new(Shown::new((Rc::clone(&self.0), mounted)))),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let shown = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Shown>())
            .ok_or_else(|| ToolUiError::Presentation("Missing difference state".into()))?;
        let (input, mounted) = &mut *shown.borrow_mut();
        if **input == *self.0 {
            return Ok(());
        }
        let lines = prepare(Rc::clone(&self.0));
        mounted.reconcile(&build(&view.document, &lines)?, &lines)?;
        *input = Rc::clone(&self.0);
        Ok(())
    }
}

/// Build the optional title and semantically colored count spans.
fn mount_header(
    document: &web_sys::Document,
    lines: &Lines,
    mounted: &mut Mounted,
) -> Result<(), attachment_content::AttachmentHostError> {
    use attachment_content::browser::{append, element};
    if let Some(title) = &lines.input.title {
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
        for row in self.rows.drain(lines.input.rows.len()..) {
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
            .set_class_name(&format!("maestro-diff {}", lines.input.class_name));
        if let Some((node, title)) = self.title.as_ref().zip(lines.input.title.as_ref()) {
            set_text_if_changed(node, title);
        }
        if let Some((added, removed)) = &self.counts {
            set_text_if_changed(added, &format!("+{}", lines.added));
            set_text_if_changed(removed, &format!("-{}", lines.removed));
        }
        for (node, row) in self.rows.iter().zip(&lines.input.rows) {
            node.set_class_name(match row.sign {
                '+' => "maestro-diff-added",
                '-' => "maestro-diff-removed",
                _ => "",
            });
            set_text_if_changed(node, &format!("{} {}", row.sign, row.text));
        }
    }
}

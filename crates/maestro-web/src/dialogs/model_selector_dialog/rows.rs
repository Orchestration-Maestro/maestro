//! Model rows retained by model identity while the list is filtered and reordered.
use super::search::{ModelEntry, format_tokens};
use crate::{
    browser::attachment_content::browser::{append, element, host_error},
    dom::{DomView, icon},
    format_model_cost,
    tools::types::ToolUiError,
};
use maestro_app::{Model, ModelInput, models_are_equal};
use std::collections::HashMap;
use web_sys::{Element, Node};

/// One mounted row and the identity it shows.
struct Row {
    /// Provider name and model identifier.
    key: (String, String),
    /// Row element.
    element: Element,
    /// Current-model mark, attached only on the current model's row.
    current: Element,
}
impl Row {
    /// Show or hide the selection highlight.
    fn select(&self, selected: bool) {
        let class = if selected {
            "maestro-model-row maestro-model-selected"
        } else {
            "maestro-model-row"
        };
        if self.element.class_name() != class {
            self.element.set_class_name(class);
        }
    }
    /// Show or hide the current-model mark.
    fn mark_current(&self, current: bool) -> Result<(), ToolUiError> {
        if !current {
            self.current.remove();
        } else if self.current.parent_node().is_none()
            && let Some(name) = self
                .element
                .query_selector(".maestro-model-name")
                .map_err(|e| host_error(&e))?
        {
            append(&name, &self.current)?;
        }
        Ok(())
    }
}
/// The rows of the scrollable list, in shown order.
pub(super) struct Rows {
    /// Scrollable list element.
    list: Element,
    /// Shown rows.
    rows: Vec<Row>,
}
impl Rows {
    /// An empty list.
    pub(super) fn new(list: Element) -> Self {
        Self {
            list,
            rows: Vec::new(),
        }
    }
    /// The scrollable list element.
    pub(super) fn list(&self) -> &Element {
        &self.list
    }
    /// Show `entries` in order, reusing the row of each identity already shown.
    /// # Errors
    /// Returns a localization, icon or native failure.
    pub(super) fn show(
        &mut self,
        view: &DomView,
        entries: &[ModelEntry],
        current: Option<&Model>,
        selected: usize,
    ) -> Result<(), ToolUiError> {
        let mut previous: HashMap<(String, String), Vec<Row>> = HashMap::new();
        for row in std::mem::take(&mut self.rows).into_iter().rev() {
            previous.entry(row.key.clone()).or_default().push(row);
        }
        let mut rows = Vec::with_capacity(entries.len());
        for entry in entries {
            let key = (entry.provider.clone(), entry.id.clone());
            let row = match previous.get_mut(&key).and_then(Vec::pop) {
                Some(row) => row,
                None => row(view, entry, key)?,
            };
            rows.push(row);
        }
        for retired in previous.into_values().flatten() {
            retired.element.remove();
        }
        for (index, (row, entry)) in rows.iter().zip(entries).enumerate() {
            row.select(index == selected);
            row.mark_current(models_are_equal(current, Some(&entry.model)))?;
            let at = self
                .list
                .child_nodes()
                .item(u32::try_from(index).unwrap_or(u32::MAX));
            if !at
                .as_ref()
                .is_some_and(|at| at.is_same_node(Some(&row.element)))
            {
                self.list
                    .insert_before(&row.element, at.as_ref())
                    .map_err(|e| host_error(&e))?;
            }
        }
        self.rows = rows;
        Ok(())
    }
    /// The shown position of the row containing `node`.
    pub(super) fn position(&self, node: &Node) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.element.contains(Some(node)))
    }
    /// The row element at `index`.
    pub(super) fn row(&self, index: usize) -> Option<&Element> {
        self.rows.get(index).map(|row| &row.element)
    }
}
/// Mount one row: identifier, provider badge, capability icons, limits and prices.
fn row(view: &DomView, entry: &ModelEntry, key: (String, String)) -> Result<Row, ToolUiError> {
    let document = &view.document;
    let root = element(document, "div", "maestro-model-row")?;
    root.set_attribute("data-model-item", "")
        .map_err(|e| host_error(&e))?;
    let top = element(document, "div", "maestro-model-top")?;
    let name = element(document, "div", "maestro-model-name")?;
    let id = element(document, "span", "maestro-model-id")?;
    id.set_text_content(Some(&entry.id));
    append(&name, &id)?;
    let current = element(document, "span", "maestro-model-current")?;
    current.set_text_content(Some("✓"));
    let badge = element(document, "span", "maestro-badge maestro-badge-outline")?;
    badge.set_text_content(Some(&entry.provider));
    append(&top, &name)?;
    append(&top, &badge)?;
    let meta = element(document, "div", "maestro-model-meta")?;
    let capabilities = element(document, "div", "maestro-model-capabilities")?;
    let model = &entry.model;
    for (glyph, supported) in [
        ("Brain", model.reasoning),
        ("Image", model.input.contains(&ModelInput::Image)),
    ] {
        let class = if supported {
            ""
        } else {
            "maestro-model-unsupported"
        };
        append(&capabilities, &icon(document, glyph, class)?)?;
    }
    let limits = element(document, "span", "")?;
    limits.set_text_content(Some(&format!(
        "{}/{}",
        format_tokens(model.context_window),
        format_tokens(model.max_tokens)
    )));
    append(&capabilities, &limits)?;
    let cost = element(document, "span", "")?;
    cost.set_text_content(Some(&format_model_cost(Some(&model.cost), &view.i18n)?));
    append(&meta, &capabilities)?;
    append(&meta, &cost)?;
    append(&root, &top)?;
    append(&root, &meta)?;
    Ok(Row {
        key,
        element: root,
        current,
    })
}

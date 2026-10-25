//! Native artifact logs with literal values and independent disclosure state.
use crate::{
    browser::{
        attachment_content::browser::{element, set_text_if_changed},
        event_subscription::EventSubscription,
        sandbox_value::BrowserSandboxValue,
    },
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::Element;

mod values;
use values::{failure, value_text};

/// Kind of a console line.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogType {
    /// Ordinary output.
    Log,
    /// Error output.
    Error,
}
impl LogType {
    /// Literal prefix of a line.
    fn label(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Error => "error",
        }
    }
}
/// One line retaining its original native operand.
#[derive(Clone)]
pub struct LogEntry {
    /// Semantic line kind.
    pub r#type: LogType,
    /// Native value, without a JSON conversion.
    pub text: BrowserSandboxValue,
}
/// Artifact output mounted directly as native page content.
#[derive(Default)]
pub struct Console {
    /// Replaceable collection of ordered lines.
    pub logs: Rc<RefCell<Vec<LogEntry>>>,
    /// Disclosure selection shared with mounted actions.
    expanded: Rc<Cell<bool>>,
}
impl Console {
    /// Create an independent empty, collapsed console.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
/// Operands replaced together when a later controller updates a root.
#[derive(Clone)]
struct Binding {
    /// Current collection, not necessarily the creator's.
    logs: Rc<RefCell<Vec<LogEntry>>>,
    /// Current disclosure selection.
    expanded: Rc<Cell<bool>>,
    /// Current page and localization.
    view: DomView,
}
/// One owned native activation registration.
type Click = EventSubscription<Closure<dyn Fn()>>;
/// Nodes and action resources retained by one mounted body.
struct Mounted {
    /// Current controller operands.
    binding: RefCell<Binding>,
    /// Summary button retaining keyboard focus.
    summary: Element,
    /// Summary label retaining its text node.
    label: Element,
    /// Glyph holder independent of the label.
    glyph: Element,
    /// Ordered positional row container.
    rows: Element,
    /// Native event registrations with weak owner backedges.
    clicks: RefCell<Vec<Click>>,
}
impl DomContent for Console {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let root = element(document, "div", "maestro-artifact-console")?;
        let header = element(document, "div", "maestro-artifact-console-header")?;
        let summary = element(document, "button", "maestro-artifact-summary")?;
        let glyph = element(document, "span", "")?;
        let label = element(document, "span", "")?;
        let rows = element(document, "div", "maestro-artifact-logs")?;
        summary
            .append_child(&glyph)
            .map_err(|error| failure(&error))?;
        summary
            .append_child(&label)
            .map_err(|error| failure(&error))?;
        header
            .append_child(&summary)
            .map_err(|error| failure(&error))?;
        root.append_child(&header)
            .map_err(|error| failure(&error))?;
        root.append_child(&rows).map_err(|error| failure(&error))?;
        let mounted = Rc::new(Mounted {
            binding: RefCell::new(self.binding(view)),
            summary,
            label,
            glyph,
            rows,
            clicks: RefCell::default(),
        });
        let weak = Rc::downgrade(&mounted);
        let callback = Closure::new(move || {
            if let Some(owner) = weak.upgrade() {
                let binding = owner.binding.borrow().clone();
                binding.expanded.set(!binding.expanded.get());
                // Native callbacks have no error channel; a failed render keeps its reached nodes.
                let _ = owner.render();
            }
        });
        let subscription =
            EventSubscription::new(mounted.summary.clone().into(), "click", callback)
                .map_err(|error| failure(&error))?;
        mounted.clicks.borrow_mut().push(subscription);
        mounted.render()?;
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(mounted)),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Rc<Mounted>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing artifact console state".into()))?;
        let previous = mounted.binding.replace(self.binding(view));
        drop(previous);
        mounted.render()
    }
}
impl Console {
    /// Capture handles without retaining the controller itself.
    fn binding(&self, view: &DomView) -> Binding {
        Binding {
            logs: Rc::clone(&self.logs),
            expanded: Rc::clone(&self.expanded),
            view: view.clone(),
        }
    }
}
impl Mounted {
    /// Reconcile summary and positional literal rows without replacing unchanged text.
    fn render(&self) -> Result<(), ToolUiError> {
        let binding = self.binding.borrow().clone();
        let logs = binding.logs.borrow().clone();
        let errors = logs
            .iter()
            .filter(|line| line.r#type == LogType::Error)
            .count();
        let count = if errors == 0 {
            logs.len().to_string()
        } else {
            format!("{errors} {}", if errors == 1 { "error" } else { "errors" })
        };
        set_text_if_changed(
            &self.label,
            &format!("{} ({count})", binding.view.i18n.text("console")?),
        );
        let expanded = binding.expanded.get();
        self.summary
            .set_attribute("aria-expanded", if expanded { "true" } else { "false" })
            .map_err(|error| failure(&error))?;
        icon(
            &self.glyph,
            if expanded {
                "ChevronDown"
            } else {
                "ChevronRight"
            },
        )?;
        self.rows
            .unchecked_ref::<web_sys::HtmlElement>()
            .set_hidden(!expanded);
        let visible = if expanded { logs.as_slice() } else { &[] };
        self.render_rows(visible, &binding.view.document)
    }
    /// Keep rows positional, removing only the no-longer-published suffix.
    fn render_rows(
        &self,
        logs: &[LogEntry],
        document: &web_sys::Document,
    ) -> Result<(), ToolUiError> {
        for (index, line) in logs.iter().enumerate() {
            let index = u32::try_from(index)
                .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
            let row = if let Some(row) = self.rows.child_nodes().item(index) {
                row.unchecked_into::<Element>()
            } else {
                let row = element(document, "div", "")?;
                self.rows
                    .append_child(&row)
                    .map_err(|error| failure(&error))?;
                row
            };
            row.set_class_name(if line.r#type == LogType::Error {
                "maestro-artifact-error"
            } else {
                "maestro-artifact-log"
            });
            set_text_if_changed(
                &row,
                &format!("[{}] {}", line.r#type.label(), value_text(&line.text.0)?),
            );
        }
        while self.rows.child_nodes().length() as usize > logs.len() {
            if let Some(row) = self.rows.last_child() {
                self.rows
                    .remove_child(&row)
                    .map_err(|error| failure(&error))?;
            }
        }
        Ok(())
    }
}
/// Reconcile a glyph without replacing an unchanged native node.
fn icon(holder: &Element, name: &str) -> Result<(), ToolUiError> {
    if holder.get_attribute("data-icon").as_deref() != Some(name) {
        let glyph = crate::browser::attachment_content::glyph_node(name)?;
        let glyph = crate::icons::icon_dom(&glyph, crate::icons::IconSize::Sm, None)
            .map_err(|error| failure(&error))?;
        holder.replace_children_with_node_1(&glyph);
        holder
            .set_attribute("data-icon", name)
            .map_err(|error| failure(&error))?;
    }
    Ok(())
}

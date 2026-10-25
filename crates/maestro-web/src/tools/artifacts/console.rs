//! Native artifact logs with literal values and independent disclosure state.
use crate::{
    browser::{
        attachment_content::browser::{element, set_text_if_changed},
        event_subscription::EventSubscription,
        sandbox_value::{BrowserSandboxValue, native_string},
    },
    components::copy_button::dom::{self as copy_button, Localization},
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
use values::value_text;

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
pub struct Console {
    /// Replaceable collection of ordered lines.
    pub logs: Rc<RefCell<Vec<LogEntry>>>,
    /// Disclosure selection shared with mounted actions.
    expanded: Rc<Cell<bool>>,
    /// Whether expanded updates follow the final row.
    autoscroll: Rc<Cell<bool>>,
}
impl Default for Console {
    fn default() -> Self {
        Self {
            logs: Rc::default(),
            expanded: Rc::default(),
            autoscroll: Rc::new(Cell::new(true)),
        }
    }
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
    /// Retained current controller state, sharing only its supplied handles.
    console: Rc<Console>,
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
    /// Header receiving expanded controls.
    header: Element,
    /// Following button, detached while collapsed.
    follow: Element,
    /// Copy element, released on collapse and owner teardown.
    copy: RefCell<Option<Element>>,
    /// Replaceable labels used by the shared copy element.
    i18n: Localization,
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
        let follow = element(document, "button", "maestro-artifact-follow")?;
        summary
            .append_child(&glyph)
            .map_err(|error| ToolUiError::native(&error))?;
        summary
            .append_child(&label)
            .map_err(|error| ToolUiError::native(&error))?;
        header
            .append_child(&summary)
            .map_err(|error| ToolUiError::native(&error))?;
        root.append_child(&header)
            .map_err(|error| ToolUiError::native(&error))?;
        root.append_child(&rows)
            .map_err(|error| ToolUiError::native(&error))?;
        let mounted = Rc::new(Mounted {
            binding: RefCell::new(self.binding(view)),
            summary,
            label,
            glyph,
            rows,
            header,
            follow,
            copy: RefCell::default(),
            i18n: Rc::new(RefCell::new(Rc::clone(&view.i18n))),
            clicks: RefCell::default(),
        });
        mounted.subscribe(&mounted.summary, false)?;
        mounted.subscribe(&mounted.follow, true)?;
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
    /// Format each whole native operand once in log order.
    pub(crate) fn get_logs_text(&self) -> Result<String, ToolUiError> {
        let logs = self.logs.borrow().clone();
        logs.iter()
            .map(|line| {
                Ok(format!(
                    "[{}] {}",
                    line.r#type.label(),
                    native_string(&line.text.0).map_err(|error| ToolUiError::native(&error))?
                ))
            })
            .collect::<Result<Vec<_>, ToolUiError>>()
            .map(|lines| lines.join("\n"))
    }
    /// Capture handles without retaining the controller itself.
    fn binding(&self, view: &DomView) -> Binding {
        Binding {
            console: Rc::new(Self {
                logs: Rc::clone(&self.logs),
                expanded: Rc::clone(&self.expanded),
                autoscroll: Rc::clone(&self.autoscroll),
            }),
            view: view.clone(),
        }
    }
}
impl Mounted {
    /// Register one activation that reads its current controller outside the binding borrow.
    fn subscribe(self: &Rc<Self>, button: &Element, following: bool) -> Result<(), ToolUiError> {
        let weak = Rc::downgrade(self);
        let callback = Closure::new(move || {
            let Some(owner) = weak.upgrade() else {
                return;
            };
            let binding = owner.binding.borrow().clone();
            let flag = if following {
                &binding.console.autoscroll
            } else {
                &binding.console.expanded
            };
            flag.set(!flag.get());
            // Native callbacks have no error channel; a failed render keeps its reached nodes.
            let _ = owner.render();
        });
        let subscription = EventSubscription::new(button.clone().into(), "click", callback)
            .map_err(|error| ToolUiError::native(&error))?;
        self.clicks.borrow_mut().push(subscription);
        Ok(())
    }
    /// Publish expanded controls through the existing copy element and its feedback owner.
    fn controls(&self, binding: &Binding) -> Result<(), ToolUiError> {
        let previous = self.i18n.replace(Rc::clone(&binding.view.i18n));
        drop(previous);
        if !binding.console.expanded.get() {
            self.follow.remove();
            let copy = self.copy.borrow_mut().take();
            if let Some(copy) = copy {
                copy.remove();
            }
            return Ok(());
        }
        let following = binding.console.autoscroll.get();
        let title = binding.view.i18n.text(if following {
            "Autoscroll enabled"
        } else {
            "Autoscroll disabled"
        })?;
        self.follow
            .set_attribute("title", &title)
            .map_err(|error| ToolUiError::native(&error))?;
        self.follow
            .set_attribute("aria-pressed", if following { "true" } else { "false" })
            .map_err(|error| ToolUiError::native(&error))?;
        icon(
            &self.follow,
            if following { "ChevronsDown" } else { "Lock" },
        )?;
        if self.follow.parent_node().is_none() {
            self.header
                .append_child(&self.follow)
                .map_err(|error| ToolUiError::native(&error))?;
        }
        let previous = self.copy.borrow().clone();
        let copy = if let Some(copy) = previous {
            copy
        } else {
            let copy = copy_button::create(
                &binding.view.document,
                &self.i18n,
                copy_button::FEEDBACK_MS,
                None,
            )
            .map_err(|error| ToolUiError::native(&error))?;
            self.copy.replace(Some(copy.clone()));
            self.header
                .append_child(&copy)
                .map_err(|error| ToolUiError::native(&error))?;
            copy
        };
        copy.set_attribute("text", &binding.console.get_logs_text()?)
            .map_err(|error| ToolUiError::native(&error))?;
        copy.set_attribute("title", &binding.view.i18n.text("Copy logs")?)
            .map_err(|error| ToolUiError::native(&error))?;
        Ok(())
    }
    /// Reconcile summary and positional literal rows without replacing unchanged text.
    fn render(&self) -> Result<(), ToolUiError> {
        let binding = self.binding.borrow().clone();
        let logs = binding.console.logs.borrow().clone();
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
        let expanded = binding.console.expanded.get();
        self.summary
            .set_attribute("aria-expanded", if expanded { "true" } else { "false" })
            .map_err(|error| ToolUiError::native(&error))?;
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
        self.controls(&binding)?;
        self.render_rows(visible, &binding.view.document)?;
        if expanded && binding.console.autoscroll.get() {
            crate::components::console_block::scroll_bottom(&self.rows);
        }
        Ok(())
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
                    .map_err(|error| ToolUiError::native(&error))?;
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
                    .map_err(|error| ToolUiError::native(&error))?;
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
            .map_err(|error| ToolUiError::native(&error))?;
        holder.replace_children_with_node_1(&glyph);
        holder
            .set_attribute("data-icon", name)
            .map_err(|error| ToolUiError::native(&error))?;
    }
    Ok(())
}

impl Drop for Mounted {
    fn drop(&mut self) {
        if let Some(copy) = self.copy.get_mut().take() {
            copy.remove();
        }
    }
}

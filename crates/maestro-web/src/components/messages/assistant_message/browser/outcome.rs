//! Usage actions and literal outcomes mounted independently of assistant content.
use super::{AssistantMessage, Outcome, Placed};
use crate::{
    browser::{
        attachment_content::browser::{element, set_text_if_changed},
        event_subscription::EventSubscription,
    },
    components::{
        button::{ButtonProps, ButtonSize, ButtonVariant, button},
        messages::AbortedMessage,
    },
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::Element;

impl<D> AssistantMessage<D> {
    /// Reconcile outcomes from the last slot backward so newly admitted usage stays first.
    pub(super) fn outcome(
        &self,
        root: &DomRoot,
        placed: &mut Placed,
        outcome: Outcome,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        if outcome.stop == maestro_app::StopReason::Aborted {
            placed.abort.show(&AbortedMessage, &root.node, view)?;
        } else {
            placed.abort.clear();
        }
        if let Some(text) = outcome.error {
            let error = ErrorMessage {
                prefix: view.i18n.text("Error:")?,
                text,
            };
            placed
                .error
                .show_before(&error, &root.node, placed.abort.root(), view)?;
        } else {
            placed.error.clear();
        }
        if let Some(text) = outcome.usage {
            let usage = UsageLine {
                text,
                action: self.on_cost_click.clone(),
            };
            placed.usage.show_before(
                &usage,
                &root.node,
                placed.error.root().or_else(|| placed.abort.root()),
                view,
            )?;
        } else {
            placed.usage.clear();
        }
        Ok(())
    }
}
/// Current optional cost callback.
type Action = Option<Rc<dyn Fn()>>;
/// Nonstreaming usage with its independently selected current action.
struct UsageLine {
    /// Delegated usage formatting.
    text: String,
    /// Current caller-owned action.
    action: Action,
}
/// A button's retained current callback and the subscription released with its root.
struct UsageState {
    /// Replaced before the next activation.
    action: Rc<RefCell<Action>>,
    /// Native click subscription, absent for plain usage.
    _click: Option<EventSubscription<Closure<dyn Fn()>>>,
}
impl DomContent for UsageLine {
    fn key(&self) -> Option<&str> {
        Some(if self.action.is_some() {
            "action"
        } else {
            "plain"
        })
    }
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let clickable = self.action.is_some();
        let class = if clickable {
            "maestro-message-usage maestro-assistant-usage maestro-text-action"
        } else {
            "maestro-message-usage maestro-assistant-usage"
        };
        let node: Element = if clickable {
            button(
                &view.document,
                &ButtonProps {
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    class_name: class,
                    ..ButtonProps::default()
                },
                &[],
            )
            .map_err(|error| ToolUiError::native(&error))?
            .into()
        } else {
            element(&view.document, "div", class)?
        };
        let action = Rc::new(RefCell::new(self.action.clone()));
        let click = if clickable {
            let current = Rc::clone(&action);
            let callback = Closure::<dyn Fn()>::new(move || invoke_action(&current));
            Some(
                EventSubscription::new(node.clone().into(), "click", callback)
                    .map_err(|error| ToolUiError::native(&error))?,
            )
        } else {
            None
        };
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(UsageState {
                action,
                _click: click,
            })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        let state = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<UsageState>())
            .ok_or_else(|| ToolUiError::Presentation("Missing usage state".into()))?;
        let previous = state.action.replace(self.action.clone());
        drop(previous);
        set_text_if_changed(root.node.unchecked_ref(), &self.text);
        Ok(())
    }
}
/// Read the current action outside its guard before invoking caller code.
fn invoke_action(current: &RefCell<Action>) {
    let action = current.borrow().clone();
    if let Some(action) = action {
        action();
    }
}
/// Literal error with a bold localized prefix.
struct ErrorMessage {
    /// Localized error prefix.
    prefix: String,
    /// Unescaped source rendered as text, never markup.
    text: String,
}
impl DomContent for ErrorMessage {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot::new(element(
            &view.document,
            "div",
            "maestro-message-error maestro-assistant-error",
        )?);
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let prefix = element(&view.document, "strong", "")?;
        prefix.set_text_content(Some(&self.prefix));
        let text = view.document.create_text_node(&format!(" {}", self.text));
        let node: &Element = root.node.unchecked_ref();
        node.replace_children_with_node_2(&prefix, &text);
        Ok(())
    }
}

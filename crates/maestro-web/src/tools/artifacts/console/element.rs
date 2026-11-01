//! Source-owned artifact console registration with native-value log properties.
use super::{Console, LogEntry, LogType, Mounted, mounted};
use crate::{
    browser::sandbox_value::BrowserSandboxValue,
    dom::{self, DomView, Getter, Link, Setter},
    tools::types::ToolUiError,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, JsValue};

/// Non-owning mounted console link shared with its property callbacks.
type Body = Rc<RefCell<Weak<Mounted>>>;
/// Bare console instance, with its source fields separate from connected nodes.
struct Element {
    /// Weak page element.
    root: Link,
    /// Page operands for bare construction.
    view: Rc<DomView>,
    /// Current native controller while a body is mounted.
    body: Body,
    /// Default console preserves disclosure across reconnection.
    console: Console,
    /// Default native rows and event subscriptions while connected.
    owned: Option<Rc<Mounted>>,
    /// Log property callables.
    _logs: (Getter, Setter),
}
impl custom_element::CustomElement for Element {
    fn connected_callback(&mut self) {
        if self.body.borrow().upgrade().is_some() {
            return;
        }
        let Some(root) = dom::live(&self.root) else {
            return;
        };
        root.replace_children_with_node_0();
        let Ok(root) = self.console.build(&self.view, root.unchecked_into()) else {
            return;
        };
        if let Ok(mounted) = mounted(&root) {
            *self.body.borrow_mut() = Rc::downgrade(mounted);
            self.owned = Some(Rc::clone(mounted));
        }
    }
    fn disconnected_callback(&mut self) {
        if self.owned.take().is_some()
            && let Some(root) = dom::live(&self.root)
        {
            root.replace_children_with_node_0();
        }
    }
}
/// Accept plain log records while retaining every text operand as its native value.
fn entries(value: &JsValue) -> Result<Vec<LogEntry>, JsValue> {
    if !js_sys::Array::is_array(value) {
        return Err(JsValue::from_str("Console logs must be an array"));
    }
    js_sys::Array::from(value)
        .iter()
        .map(|entry| {
            let kind = js_sys::Reflect::get(&entry, &"type".into())?;
            let r#type = match kind.as_string().as_deref() {
                Some("error") => LogType::Error,
                Some("log") => LogType::Log,
                _ => return Err(JsValue::from_str("Console log type must be log or error")),
            };
            Ok(LogEntry {
                r#type,
                text: BrowserSandboxValue(js_sys::Reflect::get(&entry, &"text".into())?),
            })
        })
        .collect()
}
/// Project the public log records without converting native values through JSON.
fn values(logs: &[LogEntry]) -> JsValue {
    let array = js_sys::Array::new();
    for line in logs {
        let record = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&record, &"type".into(), &line.r#type.label().into());
        let _ = js_sys::Reflect::set(&record, &"text".into(), &line.text.0);
        array.push(&record);
    }
    array.into()
}
/// Register the console tag using the same body and controls consumed by HTML artifacts.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), ToolUiError> {
    let view = Rc::clone(view);
    dom::define_element("artifact-console", vec![], &[], move |root, _| {
        let body = dom::take_creating::<Body>().unwrap_or_default();
        let console = Console::new();
        let logs = Rc::clone(&console.logs);
        let (reading, writing) = (Rc::clone(&body), Rc::clone(&body));
        let read_logs = Rc::clone(&logs);
        let get = Getter::new(move || {
            let mounted = reading.borrow().upgrade();
            if let Some(mounted) = mounted {
                values(&mounted.binding.borrow().console.logs.borrow())
            } else {
                values(&read_logs.borrow())
            }
        });
        let set = Setter::new(move |value: JsValue| {
            let lines = entries(&value).unwrap_or_else(|error| wasm_bindgen::throw_val(error));
            let mounted = writing.borrow().upgrade();
            let Some(mounted) = mounted else {
                *logs.borrow_mut() = lines;
                return;
            };
            *mounted.binding.borrow().console.logs.borrow_mut() = lines;
            mounted
                .render()
                .unwrap_or_else(|error| wasm_bindgen::throw_str(&error.to_string()));
        });
        let _ = dom::install(&root, "logs", &get, &set);
        Element {
            root: Link::new(&root),
            view: Rc::clone(&view),
            body,
            console,
            owned: None,
            _logs: (get, set),
        }
    })
    .map_err(|error| ToolUiError::native(&error))
}

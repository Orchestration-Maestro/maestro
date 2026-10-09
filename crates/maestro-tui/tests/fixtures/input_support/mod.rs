#![cfg(test)]

use maestro_tui::{
    KeybindingsManager, get_keybindings, is_kitty_protocol_active, set_keybindings,
    set_kitty_protocol_active,
};
use std::sync::{Mutex, MutexGuard};

/// Serializes every input test accessing process-wide state.
static GLOBALS: Mutex<()> = Mutex::new(());

/// Saved process globals held for one test.
pub struct Globals {
    /// Lock shared by default, custom-binding and protocol tests.
    _lock: MutexGuard<'static, ()>,
    /// Previously active manager.
    bindings: KeybindingsManager,
    /// Previously active keyboard mode.
    protocol: bool,
}

/// Starts with default bindings and restores the previous globals on drop.
pub fn globals() -> Globals {
    let lock = GLOBALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let guard = Globals {
        _lock: lock,
        bindings: get_keybindings(),
        protocol: is_kitty_protocol_active(),
    };
    set_keybindings(KeybindingsManager::new(
        maestro_tui::TUI_KEYBINDINGS.clone(),
        Vec::new(),
    ));
    set_kitty_protocol_active(false);
    guard
}

impl Drop for Globals {
    fn drop(&mut self) {
        set_keybindings(self.bindings.clone());
        set_kitty_protocol_active(self.protocol);
    }
}

use maestro_tui::{
    Component, Focusable, Input, KeybindingKeys, KeybindingsConfig, tui::InputHandler,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Retained callable from the widget's public getter.
type Submit = Rc<dyn Fn(&str)>;

/// One public widget and its callback observations.
struct Scenario {
    /// Shared widget for callback reentry.
    input: Rc<Input>,
    /// Synchronous event log.
    events: Rc<RefCell<Vec<Value>>>,
    /// An explicitly retained callback.
    retained: Option<Submit>,
}

/// Converts fixture binding records into the existing typed registry input.
fn config(value: &Value) -> KeybindingsConfig {
    value
        .as_object()
        .unwrap()
        .iter()
        .map(|(action, keys)| {
            let keys = if let Some(key) = keys.as_str() {
                KeybindingKeys::Single(key.into())
            } else {
                KeybindingKeys::Multiple(
                    keys.as_array()
                        .unwrap()
                        .iter()
                        .map(|key| key.as_str().unwrap().into())
                        .collect(),
                )
            };
            (action.clone(), Some(keys))
        })
        .collect()
}

impl Scenario {
    /// Performs one fixture operation, returning its public observation.
    fn execute(&mut self, operation: &Value) -> Value {
        let name = operation[0].as_str().unwrap();
        let argument = &operation[1];
        match name {
            "set" => self.input.set_value(argument.as_str().unwrap().into()),
            "key" => self.input.handle_input(argument.as_str().unwrap()),
            "focus" => self.input.focus_flag().set(argument.as_bool().unwrap()),
            "invalidate" => self.input.invalidate(),
            "protocol" => set_kitty_protocol_active(argument.as_bool().unwrap()),
            "bindings" => get_keybindings().set_user_bindings(config(argument)),
            "manager" => set_keybindings(KeybindingsManager::new(
                maestro_tui::TUI_KEYBINDINGS.clone(),
                config(argument),
            )),
            "render" => {
                return json!({"line": self.input.render(usize::try_from(argument.as_u64().unwrap()).unwrap())[0]});
            }
            "observe" => {}
            "retain_submit" => self.retained = self.input.on_submit(),
            "call_retained" => self.retained.as_ref().unwrap()(argument.as_str().unwrap()),
            "callbacks" => self.callbacks(argument.as_bool().unwrap()),
            "reentrant_submit" => self.reentrant_submit(),
            "reentrant_escape" => self.reentrant_escape(),
            "paste_submit" => self.paste_submit(),
            _ => panic!("unknown operation {name}"),
        }
        json!({"value": self.input.get_value()})
    }

    /// Installs or removes ordinary synchronous callback witnesses.
    fn callbacks(&self, enabled: bool) {
        let events = self.events.clone();
        self.input.set_on_submit(enabled.then(|| {
            Rc::new(move |value: &str| events.borrow_mut().push(json!(["submit", value]))) as Submit
        }));
        let events = self.events.clone();
        self.input.set_on_escape(
            enabled.then(|| {
                Rc::new(move || events.borrow_mut().push(json!(["escape"]))) as Rc<dyn Fn()>
            }),
        );
    }

    /// Starts a fresh paste from a submit callback.
    fn paste_submit(&self) {
        let input = Rc::downgrade(&self.input);
        let events = self.events.clone();
        self.input.set_on_submit(Some(Rc::new(move |value| {
            events.borrow_mut().push(json!(["submit", value]));
            input.upgrade().unwrap().handle_input("\x1b[200~next");
        })));
    }

    /// Replaces the callable and invokes it recursively while retaining the outer argument.
    fn reentrant_submit(&self) {
        let input = Rc::downgrade(&self.input);
        let events = self.events.clone();
        self.input.set_on_submit(Some(Rc::new(move |value| {
            let input = input.upgrade().unwrap();
            events
                .borrow_mut()
                .push(json!(["submit-before", value, input.get_value()]));
            input.set_value("next".into());
            let inner_events = events.clone();
            input.set_on_submit(Some(Rc::new(move |value| {
                inner_events
                    .borrow_mut()
                    .push(json!(["submit-inner", value]));
            })));
            input.handle_input("\r");
            events
                .borrow_mut()
                .push(json!(["submit-after", value, input.get_value()]));
        })));
    }

    /// Removes the callable before reentering cancellation and insertion.
    fn reentrant_escape(&self) {
        let input = Rc::downgrade(&self.input);
        let events = self.events.clone();
        self.input.set_on_escape(Some(Rc::new(move || {
            let input = input.upgrade().unwrap();
            events
                .borrow_mut()
                .push(json!(["escape-before", input.get_value()]));
            input.set_on_escape(None);
            input.set_value("cancelled".into());
            input.handle_input("!");
            input.handle_input("\x1b");
            events
                .borrow_mut()
                .push(json!(["escape-after", input.get_value()]));
        })));
    }
}

/// Runs every distinct corpus query owned by this behavior test.
pub fn run(test: &str) {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../input_cases.json")).unwrap();
    let selected: Vec<_> = cases.iter().filter(|case| case["test"] == test).collect();
    assert!(!selected.is_empty(), "no corpus queries for {test}");
    for (index, case) in selected.iter().enumerate() {
        let _guard = globals();
        let mut scenario = Scenario {
            input: Rc::new(Input::new()),
            events: Rc::default(),
            retained: None,
        };
        let operations = case["ops"].as_array().unwrap();
        let expected = case["expected"]["outputs"].as_array().unwrap();
        assert_eq!(operations.len(), expected.len());
        for (step, (operation, expected)) in operations.iter().zip(expected).enumerate() {
            assert_eq!(
                scenario.execute(operation),
                *expected,
                "{test} query {index} operation {step}: {operation}"
            );
        }
        assert_eq!(
            json!(*scenario.events.borrow()),
            case["expected"]["events"],
            "{test} query {index} events"
        );
    }
}

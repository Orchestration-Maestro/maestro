//! Fallible focused input and host error ownership.
use crate::editor_host::{manual_runtime, recording_terminal};
use maestro_tui::test_support::runtime_spy;
use maestro_tui::{Component, TUI, TerminalImage, tui::InputHandler};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Strict recorded input and outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub test: String,
    pub input: Input,
    pub expected: Value,
}
/// Toolkit dispatch inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub gate: Option<String>,
    pub data: Option<String>,
    pub child: Option<String>,
}
/// Read exactly the rows owned by the named toolkit witness.
pub fn cases(name: &str) -> Vec<Case> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/fallback_failures.json")).unwrap();
    let rows: Vec<_> = rows.into_iter().filter(|row| row.test == name).collect();
    assert!(!rows.is_empty());
    rows
}
/// Original callback failure with a retained identity token.
#[derive(Debug, Serialize)]
pub struct Failure {
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'static str>,
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<Box<Self>>,
    #[serde(skip)]
    pub identity: Rc<()>,
}
impl Failure {
    /// Full diagnostic fields, with identity that survives the trait boundary.
    fn new(identity: Rc<()>) -> Self {
        Self {
            message: "Custom credential resolver failed",
            name: Some("CustomCredentialFailure"),
            code: "CUSTOM_FALLBACK",
            cause: Some(Box::new(Self {
                message: "resolver cause",
                name: None,
                code: "CAUSE",
                cause: None,
                identity: identity.clone(),
            })),
            identity,
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &dyn std::error::Error)
    }
}
/// Input recipient with optional capability and key-release delivery.
pub struct Receiver {
    pub events: Rc<RefCell<Vec<Value>>>,
    pub failing: bool,
    pub accepts: bool,
    pub releases: bool,
    pub identity: Rc<()>,
}
impl InputHandler for Receiver {
    fn handle_input(&self, data: &str) {
        self.events.borrow_mut().push(json!(["legacy", data]));
    }
    fn try_handle_input(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.events.borrow_mut().push(json!(["input", data]));
        if self.failing {
            Err(Box::new(Failure::new(self.identity.clone())))
        } else {
            Ok(())
        }
    }
}
impl Component for Receiver {
    fn render(&self, _: usize) -> Vec<String> {
        vec![]
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        self.accepts.then_some(self)
    }
    fn wants_key_release(&self) -> bool {
        self.releases
    }
}
/// Legacy input receiver exercising default delegation.
struct Legacy(Cell<usize>);
impl InputHandler for Legacy {
    fn handle_input(&self, _: &str) {
        self.0.set(self.0.get() + 1);
    }
}
/// Check original boxed error identity and return the complete diagnostic fields.
pub fn error_value(error: &(dyn std::error::Error + 'static), identity: &Rc<()>) -> Value {
    let error = error
        .downcast_ref::<Failure>()
        .expect("original callback error");
    assert!(Rc::ptr_eq(&error.identity, identity));
    serde_json::to_value(error).unwrap()
}
#[test]
fn focused_input_returns_failure_before_render() {
    let _keys = crate::list_support::KEYS.lock().unwrap();
    for case in cases("focused_input_returns_failure_before_render") {
        assert!(case.input.child.is_none());
        let gate = case.input.gate.unwrap();
        let terminal = recording_terminal::RecordingTerminal::new(80, 24);
        let runtime = runtime_spy::Spy::default();
        let tui = TUI::new(
            terminal.handle(),
            Rc::new(runtime.clone()),
            TerminalImage::default(),
            None,
        );
        let receiver = Rc::new(Receiver {
            events: runtime.events.clone(),
            failing: matches!(gate.as_str(), "failure" | "release-delivered"),
            accepts: gate != "no-handler",
            releases: gate == "release-delivered",
            identity: Rc::new(()),
        });
        if gate != "no-focus" {
            tui.set_focus(Some(receiver.clone()));
        }
        if gate == "listener-consumed" {
            let _remove =
                tui.add_input_listener(Rc::new(|_| maestro_tui::tui::InputListenerResult::Consume));
        }
        if gate == "listener-empty" {
            let _remove = tui.add_input_listener(Rc::new(|_| {
                maestro_tui::tui::InputListenerResult::Replace(String::new())
            }));
        }
        if gate == "debug" {
            let events = runtime.events.clone();
            tui.set_on_debug(Some(Rc::new(move || {
                events.borrow_mut().push(json!(["debug"]));
            })));
        }
        tui.start().unwrap();
        runtime.clock.settle().unwrap();
        runtime.events.borrow_mut().clear();
        terminal.send_input(&case.input.data.unwrap());
        let errors = runtime.clock.drain_futures();
        assert!(errors.len() <= 1);
        let actual = if let Some(error) = errors.first() {
            assert_eq!(runtime.clock.pending(), 0);
            json!({"branch":"error","error":error_value(error.as_ref(), &receiver.identity),"events":*runtime.events.borrow()})
        } else {
            json!({"branch":"ok","value":*runtime.events.borrow(),"events":*runtime.events.borrow()})
        };
        assert!(actual == case.expected, "recorded focused dispatch: {gate}");
        runtime.clock.settle().unwrap();
        tui.stop().unwrap();
    }
}
#[test]
fn fallible_input_default_and_host_sink_keep_original_error() {
    let legacy = Legacy(Cell::new(0));
    legacy.try_handle_input("x").unwrap();
    assert_eq!(legacy.0.get(), 1);
    let terminal = recording_terminal::RecordingTerminal::new(80, 24);
    let runtime = manual_runtime::ManualRuntime::new();
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    let receiver = Rc::new(Receiver {
        events: Rc::default(),
        failing: true,
        accepts: true,
        releases: false,
        identity: Rc::new(()),
    });
    tui.set_focus(Some(receiver.clone()));
    tui.start().unwrap();
    runtime.settle().unwrap();
    terminal.send_input("x");
    assert_eq!(runtime.futures(), 1);
    let errors = runtime.drain_futures();
    assert_eq!(errors.len(), 1);
    error_value(errors[0].as_ref(), &receiver.identity);
    assert_eq!(runtime.futures(), 0);
    assert_eq!(runtime.pending(), 0);
    assert_eq!(*receiver.events.borrow(), [json!(["input", "x"])]);
    tui.stop().unwrap();
}

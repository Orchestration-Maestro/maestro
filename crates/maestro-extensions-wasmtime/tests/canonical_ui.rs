//! Every canonical UI import forwards to the independently retained native UI.
#![cfg(test)]
use maestro_extensions::{
    Cancellation, EventError, ExtensionUIContext, ExtensionUIDialogOptions, ExtensionWidgetOptions,
    WorkingIndicatorOptions,
    types::{ExtensionUIContextPort, ExtensionUIFuture, NotificationType, WidgetContent},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Original application UI records the operands its methods actually consume.
struct Ui(Rc<RefCell<Vec<Value>>>);
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["notify", message, kind]));
        Ok(())
    }
    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["status", key, text]));
        Ok(())
    }
    fn set_working_message(&self, message: Option<String>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["working", message]));
        Ok(())
    }
    fn set_working_visible(&self, visible: bool) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["visible", visible]));
        Ok(())
    }
    fn set_working_indicator(
        &self,
        options: Option<WorkingIndicatorOptions>,
    ) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!([
            "indicator",
            options.map(|o| json!({"frames":o.frames,"intervalMs":o.interval_ms}))
        ]));
        Ok(())
    }
    fn set_hidden_thinking_label(&self, label: Option<String>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["hidden", label]));
        Ok(())
    }
    fn set_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let lines = content.map(|c| match c {
            WidgetContent::Lines(lines) => lines,
            WidgetContent::Factory(_) => panic!("expected fixed lines"),
        });
        self.0.borrow_mut().push(json!([
            "widget",
            key,
            lines,
            options.map(|o| json!({"placement":o.placement}))
        ]));
        Ok(())
    }
    fn set_title(&self, title: String) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["title", title]));
        Ok(())
    }
}

#[test]
fn canonical_ui_imports_forward_values_and_absence_to_original_ui() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(Rc::new(|_| panic!("no tails")), Rc::new(|_| panic!("no diagnostics")));
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let calls = Rc::new(RefCell::new(Vec::new()));
        runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(Ui(calls.clone())))));
        let handler = runner.get_command("bridge-observe").unwrap().command.handler;
        for (args, expected) in [
            ("ui imports", json!([["notify"," literal notification ","warning"],["status"," literal key ",""],["working"," literal working "],["visible",true],["indicator",{"frames":[],"intervalMs":2.5}],["hidden"," literal hidden "],["widget"," literal widget ",[" literal line ",""],{"placement":"belowEditor"}],["title"," literal title "]])),
            ("ui clear", json!([["notify"," literal notification ",null],["status"," literal key ",null],["working",null],["visible",false],["indicator",null],["hidden",null],["widget"," literal widget ",null,null],["title",""]])),
        ] {
            assert_eq!(handler(args.into(), runner.create_command_context()).await.unwrap_err().to_string(), "UI forwarded");
            assert_eq!(json!(std::mem::take(&mut *calls.borrow_mut())), expected);
        }
        bus.clear();
    });
}

/// Application editor records exact text and can reject a request without trapping the guest.
#[derive(Default)]
struct Editor {
    /// Actual operands received from the guest.
    calls: RefCell<Vec<Value>>,
    /// Caller-controlled rejection of the next operation.
    refuse: std::cell::Cell<bool>,
}
impl ExtensionUIContextPort for Editor {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn paste_to_editor(&self, text: String) -> Result<(), EventError> {
        self.calls.borrow_mut().push(json!(["paste", text]));
        self.result(())
    }
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.calls.borrow_mut().push(json!(["set", text]));
        self.result(())
    }
    fn get_editor_text(&self) -> Result<String, EventError> {
        self.calls.borrow_mut().push(json!(["get"]));
        self.result(" literal editor\n雪 ".into())
    }
}
impl Editor {
    /// Returns the caller's refusal unchanged once, then leaves the same UI usable.
    fn result<T>(&self, value: T) -> Result<T, EventError> {
        if self.refuse.replace(false) {
            Err("editor rejected".into())
        } else {
            Ok(value)
        }
    }
}

#[test]
fn canonical_editor_paste_forwards_exact_text_and_errors() {
    editor("paste");
}

#[test]
fn canonical_editor_set_forwards_exact_text_and_errors() {
    editor("set");
}

#[test]
fn canonical_editor_get_forwards_exact_text_and_errors() {
    editor("get");
}

/// Exercises one editor import through the real component and native UI owner.
fn editor(method: &str) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let ui = Rc::new(Editor::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        for text in [" literal input\n雪 ", ""] {
            let args = if method == "get" {
                "editor get".into()
            } else {
                format!("editor {method}:{text}")
            };
            let expected = if method == "get" {
                " literal editor\n雪 "
            } else {
                "editor forwarded"
            };
            assert_eq!(
                handler(args.clone(), runner.create_command_context())
                    .await
                    .unwrap_err()
                    .to_string(),
                expected
            );
            ui.refuse.set(true);
            assert_eq!(
                handler(args, runner.create_command_context())
                    .await
                    .unwrap_err()
                    .to_string(),
                "editor rejected"
            );
            let call = if method == "get" {
                json!(["get"])
            } else {
                json!([method, text])
            };
            assert_eq!(
                std::mem::take(&mut *ui.calls.borrow_mut()),
                [call.clone(), call]
            );
        }
        bus.clear();
    });
}

/// Application dialogs record the operands they consume and answer from a supplied list.
#[derive(Default)]
struct Dialogs {
    /// Actual operands received from the guest.
    calls: RefCell<Vec<Value>>,
    /// Answers in order; a string is a rejection message inside `Err`.
    answers: RefCell<std::collections::VecDeque<Result<Value, String>>>,
}
impl Dialogs {
    /// Records one dialog and takes its supplied answer.
    fn answer(&self, call: Value) -> Result<Value, EventError> {
        self.calls.borrow_mut().push(call);
        let answer = self.answers.borrow_mut().pop_front().unwrap();
        answer.map_err(Into::into)
    }
}
/// What a dialog's options carried, or null without options.
fn carried(opts: Option<&ExtensionUIDialogOptions>) -> Value {
    opts.map_or(Value::Null, |opts| {
        json!([
            opts.signal.as_ref().map(Cancellation::is_aborted),
            opts.timeout
        ])
    })
}
impl ExtensionUIContextPort for Dialogs {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let answer = self.answer(json!(["select", title, options, carried(opts.as_ref())]));
        Box::pin(std::future::ready(
            answer.map(|value| value.as_str().map(str::to_owned)),
        ))
    }
    fn confirm(
        &self,
        title: String,
        message: String,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let answer = self.answer(json!(["confirm", title, message, carried(opts.as_ref())]));
        Box::pin(std::future::ready(answer.map(|value| value == true)))
    }
    fn input(
        &self,
        title: String,
        placeholder: Option<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let answer = self.answer(json!(["input", title, placeholder, carried(opts.as_ref())]));
        Box::pin(std::future::ready(
            answer.map(|value| value.as_str().map(str::to_owned)),
        ))
    }
    fn editor(&self, title: String, prefill: Option<String>) -> ExtensionUIFuture<Option<String>> {
        let answer = self.answer(json!(["editor", title, prefill]));
        Box::pin(std::future::ready(
            answer.map(|value| value.as_str().map(str::to_owned)),
        ))
    }
}

/// Each dialog, its answer and the host call the original application UI received.
fn dialog_rows() -> [(Value, Result<Value, String>, &'static str, Value); 6] {
    [
        (
            json!({"method":"select","title":"Pick 雪","options":["a","","a"]}),
            Ok(json!("a")),
            "\"a\"",
            json!(["select", "Pick 雪", ["a", "", "a"], [null, null]]),
        ),
        (
            json!({"method":"confirm","title":"","message":"Sure?\nnow"}),
            Ok(json!(true)),
            "true",
            json!(["confirm", "", "Sure?\nnow", [null, null]]),
        ),
        (
            json!({"method":"input","title":"Name"}),
            Ok(Value::Null),
            "null",
            json!(["input", "Name", null, [null, null]]),
        ),
        (
            json!({"method":"input","title":"Name","placeholder":""}),
            Ok(json!("")),
            "\"\"",
            json!(["input", "Name", "", [null, null]]),
        ),
        (
            json!({"method":"editor","title":"Edit","prefill":"a\r\nb"}),
            Ok(json!("a\r\nb\nc")),
            "\"a\\r\\nb\\nc\"",
            json!(["editor", "Edit", "a\r\nb"]),
        ),
        (
            json!({"method":"select","title":"Pick","options":[]}),
            Err("dialog 雪\nrefused".to_owned()),
            "dialog 雪\nrefused",
            json!(["select", "Pick", [], [null, null]]),
        ),
    ]
}

#[test]
fn canonical_dialogs_forward_operands_answers_and_errors() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let ui = Rc::new(Dialogs::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        for (request, answer, returned, call) in dialog_rows() {
            ui.answers.borrow_mut().push_back(answer);
            let outcome = handler(format!("dialog {request}"), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string();
            assert_eq!(outcome, returned, "answer of {request}");
            assert_eq!(
                std::mem::take(&mut *ui.calls.borrow_mut()),
                [call],
                "host call of {request}"
            );
        }
        bus.clear();
    });
}

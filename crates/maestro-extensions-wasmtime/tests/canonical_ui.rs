//! Every canonical UI import forwards to the independently retained native UI.
#![cfg(test)]
use maestro_extensions::{
    Cancellation, EventError, ExtensionRunner, ExtensionUIContext, ExtensionUIDialogOptions,
    ExtensionWidgetOptions, InputListenerResult, LiveTheme, Local, TerminalInputHandler,
    ToolRenderCall, ToolRenderContext, ToolRendererBinding, ToolRendererState,
    WorkingIndicatorOptions,
    types::{
        ComponentFactory, ExtensionUIContextPort, ExtensionUIFuture, NotificationType,
        WidgetContent,
    },
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
fn dialog_rows() -> [(Value, Result<Value, String>, &'static str, Value); 9] {
    [
        (
            json!({"method":"select","title":"T","options":["a"],"timeout":-1.0}),
            Ok(json!("a")),
            "\"a\"",
            json!(["select", "T", ["a"], [null, -1.0]]),
        ),
        (
            json!({"method":"input","title":"T","aborted":true,"timeout":2.5}),
            Ok(Value::Null),
            "null",
            json!(["input", "T", null, [true, 2.5]]),
        ),
        (
            json!({"method":"confirm","title":"Stop","message":"","aborted":true}),
            Ok(json!(false)),
            "false",
            json!(["confirm", "Stop", "", [true, null]]),
        ),
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

/// Live terminal input listeners in registration order, with the identities removed so far.
#[derive(Default)]
struct Terminal {
    /// Listeners by registration identity.
    listeners: Rc<RefCell<Vec<(usize, TerminalInputHandler)>>>,
    /// Identities whose removal the guest requested.
    removed: Rc<RefCell<Vec<usize>>>,
    /// What each listener decided when a notification reentered it.
    reentered: RefCell<Vec<String>>,
}
impl Terminal {
    /// Every live listener's decision about `data`, in registration order.
    fn deliver(&self, data: &str) -> Vec<String> {
        let listeners: Vec<TerminalInputHandler> = self
            .listeners
            .borrow()
            .iter()
            .map(|(_, listener)| listener.clone())
            .collect();
        listeners
            .iter()
            .map(|listener| match listener.call(data.to_owned()) {
                Ok(InputListenerResult::Pass) => "pass".to_owned(),
                Ok(InputListenerResult::Consume) => "consume".to_owned(),
                Ok(InputListenerResult::Replace(data)) => format!("replace {data}"),
                Err(error) => format!("error {error}"),
            })
            .collect()
    }
}
impl ExtensionUIContextPort for Terminal {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(&self, _message: String, _kind: Option<NotificationType>) -> Result<(), EventError> {
        let decisions = self.deliver("reenter");
        self.reentered.borrow_mut().extend(decisions);
        Ok(())
    }
    fn on_terminal_input(
        &self,
        handler: TerminalInputHandler,
    ) -> Result<maestro_extensions::types::UIUnsubscribe, EventError> {
        let mut listeners = self.listeners.borrow_mut();
        let id = listeners.len();
        listeners.push((id, handler));
        let (listeners, removed) = (Rc::clone(&self.listeners), Rc::clone(&self.removed));
        Ok(Rc::new(move || {
            removed.borrow_mut().push(id);
            listeners.borrow_mut().retain(|(live, _)| *live != id);
            Ok(())
        }))
    }
}

#[test]
fn canonical_terminal_input_reaches_listeners_and_removal() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let ui = Rc::new(Terminal::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let run = async |args: &str| {
            handler(
                format!("ui terminal {args}"),
                runner.create_command_context(),
            )
            .await
            .unwrap_err()
            .to_string()
        };
        assert_eq!(run("listen a").await, "listen a");
        assert_eq!(run("listen b").await, "listen b");
        for (data, expected) in [
            ("x 雪", ["replace a:x 雪", "replace b:x 雪"]),
            ("consume", ["consume", "consume"]),
            ("pass", ["pass", "pass"]),
            ("fail", ["error a failed", "error b failed"]),
        ] {
            assert_eq!(ui.deliver(data), expected, "listeners decide {data:?}");
        }
        assert_eq!(run("unsubscribe a").await, "unsubscribe a");
        assert_eq!(*ui.removed.borrow(), [0]);
        assert_eq!(run("drop b").await, "drop b");
        assert_eq!(
            ui.deliver("y"),
            ["replace b:y"],
            "a dropped subscription keeps b"
        );
        handler("ui imports".into(), runner.create_command_context())
            .await
            .unwrap_err();
        assert_eq!(
            *ui.reentered.borrow(),
            ["replace b:reenter"],
            "a listener reentered from a waiting import answers"
        );
        bus.clear();
    });
}

/// Application themes published through one live view, with the tool expansion.
struct Themes {
    /// The published theme the live view reads.
    state: maestro_theme::ThemeState,
    /// Loadable themes by name, in the host's order.
    loadable: Vec<(String, Rc<maestro_theme::Theme>)>,
    /// Tool output expansion.
    expanded: std::cell::Cell<bool>,
}
impl Themes {
    /// A host listing `dark` and `light`, with `dark` published.
    fn new() -> Self {
        let theme = |name: &str| {
            let options = maestro_theme::ThemeOptions {
                name: Some(name.into()),
                source_path: (name == "dark").then(|| "/themes/dark.json".into()),
                source_info: None,
            };
            Rc::new(
                maestro_theme::Theme::new([], [], maestro_theme::ColorMode::Truecolor, options)
                    .unwrap(),
            )
        };
        let loadable = vec![
            ("light".to_owned(), theme("light")),
            ("dark".to_owned(), theme("dark")),
        ];
        let directories = maestro_theme::ThemeDirectories {
            themes_dir: String::new(),
            custom_themes_dir: String::new(),
        };
        let state = maestro_theme::ThemeState::new(directories);
        state.set_theme_instance(loadable[1].1.clone()).unwrap();
        Self {
            state,
            loadable,
            expanded: std::cell::Cell::default(),
        }
    }
    /// The loadable theme `name`.
    fn find(&self, name: &str) -> Option<Rc<maestro_theme::Theme>> {
        self.loadable
            .iter()
            .find(|(it, _)| it == name)
            .map(|(_, theme)| theme.clone())
    }
}
impl ExtensionUIContextPort for Themes {
    fn theme(&self) -> maestro_theme::LiveTheme {
        self.state.theme()
    }
    fn get_all_themes(&self) -> Result<Vec<maestro_theme::ThemeInfo>, EventError> {
        let info = |(name, theme): &(String, Rc<maestro_theme::Theme>)| maestro_theme::ThemeInfo {
            name: name.clone(),
            path: theme.source_path().map(str::to_owned),
        };
        Ok(self.loadable.iter().map(info).collect())
    }
    fn get_theme(&self, name: String) -> Result<Option<Rc<maestro_theme::Theme>>, EventError> {
        Ok(self.find(&name))
    }
    fn set_theme(
        &self,
        theme: maestro_extensions::types::ThemeSelection,
    ) -> Result<maestro_extensions::types::ThemeSetResult, EventError> {
        let theme = match theme {
            maestro_extensions::types::ThemeSelection::Name(name) => self.find(&name),
            maestro_extensions::types::ThemeSelection::Theme(live) => live.get().ok(),
        };
        let success = theme.is_some_and(|theme| self.state.set_theme_instance(theme).is_ok());
        let error = (!success).then(|| "Theme not found".to_owned());
        Ok(maestro_extensions::types::ThemeSetResult { success, error })
    }
    fn get_tools_expanded(&self) -> Result<bool, EventError> {
        Ok(self.expanded.get())
    }
    fn set_tools_expanded(&self, expanded: bool) -> Result<(), EventError> {
        self.expanded.set(expanded);
        Ok(())
    }
}

#[test]
fn canonical_themes_and_tool_expansion_reach_the_original_ui() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(Themes::new()))));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        for (request, expected) in [
            ("current", json!("dark")),
            (
                "all",
                json!([["light", null], ["dark", "/themes/dark.json"]]),
            ),
            ("load light", json!("light")),
            ("current", json!("dark")),
            ("load missing", Value::Null),
            ("select light", json!([true, null])),
            ("current", json!("light")),
            ("select missing", json!([false, "Theme not found"])),
            ("select-loaded dark", json!([true, null])),
            ("current", json!("dark")),
            ("expanded", json!(false)),
            ("expand true", Value::Null),
            ("expanded", json!(true)),
            ("expand false", Value::Null),
            ("expanded", json!(false)),
        ] {
            let answer = handler(
                format!("ui theme {request}"),
                runner.create_command_context(),
            )
            .await
            .unwrap_err()
            .to_string();
            assert_eq!(
                serde_json::from_str::<Value>(&answer).unwrap(),
                expected,
                "{request}"
            );
        }
        bus.clear();
    });
}

/// Application widgets build each factory as soon as it is set, like the interactive mode.
pub(super) struct Widgets {
    /// The terminal factories receive.
    pub(super) tui: maestro_tui::TUI,
    /// The theme factories receive.
    pub(super) theme: maestro_theme::LiveTheme,
    /// The component each key shows.
    pub(super) shown: RefCell<Vec<(String, maestro_extensions::types::ExtensionComponent)>>,
}
impl Widgets {
    /// Disposes every shown component, then builds `factory` when there is one.
    fn replace(&self, key: String, factory: Option<ComponentFactory>) -> Result<(), EventError> {
        let removed = self.shown.borrow_mut().drain(..).collect::<Vec<_>>();
        for (_, widget) in removed {
            if let Some(dispose) = widget.dispose {
                dispose.call(())?;
            }
        }
        if let Some(factory) = factory {
            let built = factory.call((self.tui.clone(), self.theme.clone()))?;
            self.shown.borrow_mut().push((key, built));
        }
        Ok(())
    }
}
impl ExtensionUIContextPort for Widgets {
    fn theme(&self) -> maestro_theme::LiveTheme {
        self.theme.clone()
    }
    fn set_header(&self, factory: Option<ComponentFactory>) -> Result<(), EventError> {
        self.replace("header".to_owned(), factory)
    }
    fn set_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        _options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let factory = match content {
            Some(WidgetContent::Factory(factory)) => Some(factory),
            _ => None,
        };
        self.replace(key, factory)
    }
}

#[test]
fn canonical_widget_factories_build_render_and_dispose_through_the_component() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _terminal) = screen();
        let ui = Rc::new(Widgets {
            tui,
            theme: maestro_theme::LiveTheme::default(),
            shown: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let flag = |name: &str| owner.read().unwrap().flags.contains_key(name);
        for label in ["A", "B"] {
            let shown = handler(
                format!("ui widget {label}"),
                runner.create_command_context(),
            )
            .await
            .unwrap_err()
            .to_string();
            assert_eq!(shown, "widget shown");
            let lines = ui.shown.borrow()[0].1.component.try_render(17).unwrap();
            assert_eq!(
                lines,
                [format!("{label} 17")],
                "the factory's component renders"
            );
        }
        assert!(
            flag("disposed-A"),
            "replacing A ran its disposal inside the setter"
        );
        assert!(!flag("disposed-B"));
        let removed = ui.shown.borrow_mut().pop().unwrap().1;
        removed.dispose.unwrap().call(()).unwrap();
        assert!(flag("disposed-B"));
        drop(removed.component);
        bus.clear();
    });
}

#[test]
fn canonical_header_factories_replace_dispose_and_restore() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _terminal) = screen();
        let ui = Rc::new(Widgets {
            tui,
            theme: maestro_theme::LiveTheme::default(),
            shown: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let flag = |name: &str| owner.read().unwrap().flags.contains_key(name);
        for (request, rendered) in [
            ("header H1", Some("H1 9")),
            ("header H2", Some("H2 9")),
            ("header", None),
        ] {
            let shown = handler(
                format!("ui widget {request}"),
                runner.create_command_context(),
            )
            .await
            .unwrap_err()
            .to_string();
            assert_eq!(shown, "widget shown");
            let lines = ui
                .shown
                .borrow()
                .first()
                .map(|(_, built)| built.component.try_render(9).unwrap());
            assert_eq!(
                lines,
                rendered.map(|line| vec![line.to_owned()]),
                "{request}"
            );
        }
        assert!(
            flag("disposed-H1") && flag("disposed-H2"),
            "each replaced header was disposed in the setter"
        );
        bus.clear();
    });
}

/// Footer metadata whose branch listeners the test runs.
#[derive(Default)]
pub(super) struct Metadata {
    /// Current branch.
    branch: RefCell<Option<String>>,
    /// Live extension statuses.
    statuses: maestro_extensions::footer_data_provider::ExtensionStatuses,
    /// Branch listeners in registration order.
    listeners: RefCell<Vec<Rc<dyn Fn()>>>,
}
impl maestro_extensions::footer_data_provider::ReadonlyFooterDataProvider for Metadata {
    fn get_git_branch(&self) -> Option<String> {
        self.branch.borrow().clone()
    }
    fn get_extension_statuses(
        &self,
    ) -> maestro_extensions::footer_data_provider::ExtensionStatuses {
        self.statuses.clone()
    }
    fn get_available_provider_count(&self) -> f64 {
        3.0
    }
    fn on_branch_change(&self, callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        self.listeners.borrow_mut().push(callback);
        Box::new(|| {})
    }
}

/// An application footer that builds each footer factory as soon as it is set.
struct Footers {
    /// The terminal factories receive.
    tui: maestro_tui::TUI,
    /// The metadata factories receive.
    metadata: Rc<Metadata>,
    /// The custom footer.
    shown: RefCell<Option<maestro_extensions::types::ExtensionComponent>>,
}
impl Footers {
    /// The custom footer's one rendered line, as the JSON it holds.
    fn render(&self) -> Value {
        let lines = self
            .shown
            .borrow()
            .as_ref()
            .unwrap()
            .component
            .try_render(10)
            .unwrap();
        serde_json::from_str(&lines[0]).unwrap()
    }
}
impl ExtensionUIContextPort for Footers {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn set_footer(
        &self,
        factory: Option<maestro_extensions::types::FooterFactory>,
    ) -> Result<(), EventError> {
        let footer = factory
            .map(|factory| {
                factory.call((self.tui.clone(), self.theme(), self.metadata.clone() as _))
            })
            .transpose()?;
        *self.shown.borrow_mut() = footer;
        Ok(())
    }
}

#[test]
fn canonical_footer_factories_read_live_metadata_and_branch_changes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _terminal) = screen();
        let metadata = Rc::new(Metadata::default());
        *metadata.branch.borrow_mut() = Some("main".into());
        metadata.statuses.insert("a".into(), "1".into());
        let ui = Rc::new(Footers {
            tui,
            metadata: metadata.clone(),
            shown: RefCell::default(),
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        let handler = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let shown = handler("ui widget footer F".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(shown, "widget shown");
        assert_eq!(ui.render(), json!(["F", "main", [["a", "1"]], 3.0]));
        *metadata.branch.borrow_mut() = None;
        metadata.statuses.insert("b".into(), String::new());
        let current = json!(["F", null, [["a", "1"], ["b", ""]], 3.0]);
        assert_eq!(ui.render(), current, "reads stay live");
        let listeners = metadata.listeners.borrow().clone();
        assert_eq!(listeners.len(), 1);
        listeners[0]();
        assert!(
            owner.read().unwrap().flags.contains_key("branch-F"),
            "the branch listener ran"
        );
        let restored = handler("ui widget footer".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(restored, "widget shown");
        assert!(ui.shown.borrow().is_none());
        bus.clear();
    });
}

/// A terminal writer over a recording terminal and a manual runtime.
pub(super) fn screen() -> (
    maestro_tui::TUI,
    super::recording_terminal::RecordingTerminal,
) {
    let terminal = super::recording_terminal::RecordingTerminal::new(40, 12);
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        super::manual_runtime::ManualRuntime::new().handle(),
        maestro_tui::TerminalImage::default(),
        Some(false),
    );
    (tui, terminal)
}

/// Supports no language, so rendering never classifies code.
struct Plain;
impl maestro_theme::SyntaxHighlighter for Plain {
    fn supports_language(&self, _: &str) -> bool {
        false
    }
    fn highlight(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Vec::new())
    }
}

/// One rendering's lines, parsed from the JSON each holds.
fn lines(component: &maestro_extensions::ComponentHandle) -> Vec<Value> {
    let lines = component.try_render(60).unwrap();
    lines
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Registers the bridge's painter tool and returns its registration.
async fn painter(
    runner: &maestro_extensions::ExtensionRunner<maestro_extensions::Local>,
) -> maestro_extensions::RegisteredTool<maestro_extensions::Local> {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let _ = handler("register-painter".into(), runner.create_command_context()).await;
    runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == "bridge-painter")
        .unwrap()
}

/// A row's render context: completed arguments, an expanded row and a redraw counter.
pub(super) fn row_context<'a>(
    args: &'a Value,
    rows: &'a maestro_extensions::ToolRendererState,
    redraws: &Rc<std::cell::Cell<u32>>,
) -> maestro_extensions::ToolRenderContext<'a, maestro_extensions::ToolRendererState> {
    let counted = redraws.clone();
    let environment = Rc::new(maestro_extensions::ToolRenderEnvironment {
        runtime: super::manual_runtime::ManualRuntime::new().handle(),
        readme_path: String::new(),
        live_theme: maestro_theme::LiveTheme::default(),
        highlighter: Rc::new(Plain),
        terminal_image: maestro_tui::TerminalImage::default(),
    });
    maestro_extensions::ToolRenderContext {
        args: Some(args),
        tool_call_id: "n1".into(),
        invalidate: Rc::new(move || counted.set(counted.get() + 1)),
        environment,
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/work".into(),
        phase: maestro_extensions::ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: false,
        },
        expanded: true,
        show_images: false,
        is_error: false,
    }
}

#[test]
fn canonical_tool_renderers_keep_row_state_and_read_the_row() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let tool = painter(&runner).await;
        let theme = maestro_theme::LiveTheme::default();
        let rows = maestro_extensions::ToolRendererState::default();
        let redraws = Rc::new(std::cell::Cell::new(0));
        let args = json!({ "a": 1, "redraw": true });
        let context = row_context(&args, &rows, &redraws);
        let binding = tool.definition.renderer_binding().unwrap();
        let render_call = binding.render_call.unwrap();
        let first = lines(&render_call(context.args, &theme, &context).unwrap());
        assert_eq!(
            first[0],
            json!(["call", "n1", args, "/work", true, true, 1, false])
        );
        let second = lines(&render_call(context.args, &theme, &context).unwrap());
        assert_eq!(second[0][6], json!(2), "the row's guest state persists");
        assert_eq!(
            second[0][7],
            json!(true),
            "the slot sees its previous component"
        );
        assert_eq!(redraws.get(), 2, "each rendering redrew its row");
        let details = json!({ "n": 1 });
        let result = maestro_extensions::ToolRenderResultLike {
            content: &[],
            details: Some(&details),
            terminate: None,
            is_error: Some(false),
        };
        let prepared = binding.render_result.unwrap().prepare(result).unwrap();
        let options = maestro_extensions::ToolRenderResultOptions {
            expanded: true,
            is_partial: false,
        };
        let rendered = lines(&prepared.render(options, &theme, &context).unwrap());
        assert_eq!(rendered[0][6], json!(3), "both slots share the row's state");
        assert_eq!(
            rendered[0][7],
            json!(false),
            "the result slot has no previous component yet"
        );
        assert_eq!(rendered[1]["details"], details);
        bus.clear();
    });
}

/// Runs both slots through failures and checks state and the next previous component.
fn renderer_failures(attempts: &[bool]) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let tool = painter(&runner).await;
        for slot in ["call", "result"] {
            render_failure_slot(&tool, attempts, slot);
        }
        bus.clear();
    });
}

/// Checks mutations and previous-component observations for one renderer slot.
fn render_failure_slot(
    tool: &maestro_extensions::RegisteredTool<maestro_extensions::Local>,
    attempts: &[bool],
    slot: &str,
) {
    let binding = tool.definition.renderer_binding().unwrap();
    let theme = maestro_theme::LiveTheme::default();
    let redraws = Rc::new(std::cell::Cell::new(0));
    let result = maestro_extensions::ToolRenderResultLike {
        content: &[],
        details: None,
        terminate: None,
        is_error: None,
    };
    let prepared = binding.render_result.unwrap().prepare(result).unwrap();
    let options = maestro_extensions::ToolRenderResultOptions {
        expanded: false,
        is_partial: true,
    };
    let call = binding.render_call.unwrap();
    let rows = maestro_extensions::ToolRendererState::default();
    for (attempt, &fail) in attempts.iter().enumerate() {
        let args = json!({ "fail": fail });
        let context = row_context(&args, &rows, &redraws);
        let rendered = if slot == "call" {
            call(context.args, &theme, &context)
        } else {
            prepared.render(options, &theme, &context)
        };
        if fail {
            assert_eq!(rendered.err().unwrap().message, "paint failed");
        } else {
            let report = lines(&rendered.unwrap());
            assert_eq!(
                report[0][6],
                json!(attempt + 1),
                "{slot} keeps failed mutations"
            );
            assert_eq!(
                report[0][7],
                json!(attempt > 0 && !attempts[attempt - 1]),
                "{slot} clears failed component"
            );
        }
    }
}

/// A first rendering failure must not discard the row's mutations.
#[test]
fn canonical_tool_renderer_error_first_keeps_state() {
    renderer_failures(&[true, false]);
}

/// Each failed slot clears its cached component before the next rendering.
#[test]
fn canonical_tool_renderer_failure_clears_previous_component() {
    renderer_failures(&[false, true, false, false]);
}

/// Native callback operands need not match the row context, including absent call args.
#[test]
fn canonical_tool_renderer_operands_are_independent_of_context() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let tool = painter(&runner).await;
        let binding = tool.definition.renderer_binding().unwrap();
        let theme = maestro_theme::LiveTheme::default();
        let rows = maestro_extensions::ToolRendererState::default();
        let redraws = Rc::new(std::cell::Cell::new(0));
        let args = json!({ "context": true });
        let supplied = json!({ "supplied": true });
        let context = row_context(&args, &rows, &redraws);
        let call = binding.render_call.unwrap();
        for operand in [None, Some(&supplied)] {
            let report = lines(&call(operand, &theme, &context).unwrap());
            assert_eq!(report[0][2], args);
            assert_eq!(report[1], json!(operand));
        }
        bus.clear();
    });
}

/// Missing, null and populated details stay distinct at the component boundary.
#[test]
fn canonical_tool_renderer_result_preserves_optional_fields() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let tool = painter(&runner).await;
        let renderer = tool
            .definition
            .renderer_binding()
            .unwrap()
            .render_result
            .unwrap();
        let theme = maestro_theme::LiveTheme::default();
        let rows = maestro_extensions::ToolRendererState::default();
        let redraws = Rc::new(std::cell::Cell::new(0));
        let args = json!({});
        let context = row_context(&args, &rows, &redraws);
        let null = Value::Null;
        let populated = json!({ "value": 1 });
        let flags = [None, Some(false), Some(true)];
        let pairs: Vec<_> = flags
            .into_iter()
            .flat_map(|terminate| flags.map(|is_error| (terminate, is_error)))
            .collect();
        for details in [None, Some(&null), Some(&populated)] {
            for &(terminate, is_error) in &pairs {
                let result = maestro_extensions::ToolRenderResultLike {
                    content: &[],
                    details,
                    terminate,
                    is_error,
                };
                let prepared = renderer.prepare(result).unwrap();
                let options = maestro_extensions::ToolRenderResultOptions {
                    expanded: true,
                    is_partial: false,
                };
                let report = lines(&prepared.render(options, &theme, &context).unwrap());
                assert_eq!(report[1].get("details"), details);
                assert_eq!(
                    report[1].get("terminate"),
                    terminate.map(Value::Bool).as_ref()
                );
                assert_eq!(report[1].get("isError"), is_error.map(Value::Bool).as_ref());
            }
        }
        bus.clear();
    });
}

/// Result callback options stay independent of the row context.
#[test]
fn canonical_tool_renderer_options_are_independent_of_context() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let tool = painter(&runner).await;
        let binding = tool.definition.renderer_binding().unwrap();
        let theme = maestro_theme::LiveTheme::default();
        let rows = maestro_extensions::ToolRendererState::default();
        let redraws = Rc::new(std::cell::Cell::new(0));
        let args = json!({ "context": true });
        let context = row_context(&args, &rows, &redraws);
        let result = maestro_extensions::ToolRenderResultLike {
            content: &[],
            details: None,
            terminate: None,
            is_error: None,
        };
        let prepared = binding.render_result.unwrap().prepare(result).unwrap();
        let options = maestro_extensions::ToolRenderResultOptions {
            expanded: false,
            is_partial: true,
        };
        let report = lines(&prepared.render(options, &theme, &context).unwrap());
        assert_eq!(report[0][5], json!(true), "context remains expanded");
        assert_eq!(report[2], json!([false, true]), "supplied result options");
        bus.clear();
    });
}

/// A native read delegated twice must receive exactly the object the wrapper returned.
#[test]
fn builtin_renderers_keep_native_paint_and_retained_state() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for own in [false, true] {
            for flags in [
                [false, true, false, true, false, true],
                [true, false, true, false, true, false],
            ] {
                builtin_render_identity(own, flags).await;
            }
        }
    });
}

/// Compares actual retained painting, not a serialized component identity.
async fn builtin_render_identity(own: bool, flags: [bool; 6]) {
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no diagnostics")),
    );
    let counts = render_observations(&bus, "builtin-render-count");
    let contexts = render_observations(&bus, "builtin-render-context");
    let trace = render_observations(&bus, "trace");
    let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
    let binding = builtin_binding(&runner, if own { "own" } else { "base" }).await;
    let theme = builtin_theme();
    let rows = ToolRendererState::default();
    let redraws = Rc::new(std::cell::Cell::new(0));
    let args = json!({"path":"second.rs"});
    let mut context = row_context(&args, &rows, &redraws);
    context.cwd = "/work/雪".into();
    context.tool_call_id = "paint-😀".into();
    context.phase = maestro_extensions::ToolRenderPhase {
        execution_started: flags[0],
        args_complete: flags[1],
        is_partial: flags[2],
    };
    context.expanded = flags[3];
    context.show_images = flags[4];
    context.is_error = flags[5];
    builtin_call_identity(
        binding.render_call.as_ref().unwrap(),
        &theme,
        &mut context,
        own,
    );
    builtin_result_identity(&binding, &theme, &mut context, bus.for_extension());
    assert_render_contexts(&contexts.borrow(), flags);
    assert_eq!(
        *trace.borrow(),
        ["g1:native-highlight", "g2:native-highlight"]
            .repeat(4)
            .into_iter()
            .map(Value::from)
            .collect::<Vec<_>>(),
        "the native highlighter reenters the same guest"
    );
    assert_eq!(
        *counts.borrow(),
        [json!(1), json!(2), json!(3), json!(4)],
        "both slots share guest state"
    );
    assert_eq!(redraws.get(), 4, "invalidation reaches the native row");
    bus.clear();
}

/// Reuses the actual result component while the native highlighter reenters its guest.
fn builtin_result_identity(
    binding: &ToolRendererBinding,
    theme: &LiveTheme,
    context: &mut ToolRenderContext<'_, ToolRendererState>,
    events: maestro_extensions::EventBus<Local>,
) {
    context.environment = Rc::new(maestro_extensions::ToolRenderEnvironment {
        runtime: context.environment.runtime.clone(),
        readme_path: "/work/README.md".into(),
        live_theme: theme.clone(),
        highlighter: Rc::new(ReenterHighlight(events)),
        terminal_image: context.environment.terminal_image.clone(),
    });
    context.last_component = None;
    let first = builtin_error_result(binding, theme, context, "native 雪");
    let before = first.try_render(60).unwrap();
    context.last_component = Some(first.clone());
    let second = builtin_error_result(binding, theme, context, "changed native 雪");
    let after = second.try_render(60).unwrap();
    assert_ne!(before, after);
    assert_eq!(first.try_render(60).unwrap(), after);
}

/// Every supplied phase, path and presentation flag reaches the actual author callback.
fn assert_render_contexts(observed: &[Value], flags: [bool; 6]) {
    let expected: Vec<_> = [
        ("call", "first.txt", false, Value::Null),
        ("call", "second.rs", true, Value::Null),
        ("result", "second.rs", false, json!([false, true])),
        ("result", "second.rs", true, json!([false, true])),
    ]
    .into_iter()
    .map(|(slot, path, previous, options)| {
        json!({
            "slot":slot,"args":{"path":path},"id":"paint-😀","cwd":"/work/雪",
            "started":flags[0],"complete":flags[1],"partial":flags[2],
            "expanded":flags[3],"images":flags[4],"error":flags[5],
            "previous":previous,"options":options
        })
    })
    .collect();
    assert_eq!(observed, expected);
}

/// The individual native factory supplies the complete expected call paint independently.
fn assert_native_call(
    observed: &[String],
    theme: &LiveTheme,
    context: &ToolRenderContext<'_, ToolRendererState>,
) {
    let definition = maestro_extensions::local::create_read_tool_definition::<()>(
        "/".into(),
        maestro_extensions::ReadToolOptions::default(),
    );
    let native = ToolRendererBinding::from_raw(&definition)
        .unwrap()
        .render_call
        .unwrap();
    let mut independent = context.with_state(context.state.clone());
    independent.last_component = None;
    let expected = native(context.args, theme, &independent)
        .unwrap()
        .try_render(60)
        .unwrap();
    assert_eq!(observed, expected);
}

/// Registers a real author wrapper, then reads its retained native registration.
async fn builtin_binding(runner: &ExtensionRunner<Local>, mode: &str) -> ToolRendererBinding {
    let register = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    register(
        format!("builtin-register-renderer {mode}"),
        runner.create_command_context(),
    )
    .await
    .unwrap();
    runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == "builtin-render")
        .unwrap()
        .definition
        .renderer_binding()
        .unwrap()
}

/// Returning the native component reuses it; returning an authored replacement does not.
fn builtin_call_identity(
    render: &ToolRenderCall<ToolRendererState>,
    theme: &LiveTheme,
    context: &mut ToolRenderContext<'_, ToolRendererState>,
    own: bool,
) {
    let first_args = json!({"path":"first.txt"});
    let mut first_context = context.with_state(context.state.clone());
    first_context.args = Some(&first_args);
    let first = render(first_context.args, theme, &first_context).unwrap();
    let before = first.try_render(60).unwrap();
    assert_native_call(&before, theme, &first_context);
    drop(first_context);
    context.last_component = Some(first.clone());
    let second = render(context.args, theme, context).unwrap();
    let after = second.try_render(60).unwrap();
    assert_native_call(&after, theme, context);
    assert_ne!(before, after);
    assert_eq!(
        first.try_render(60).unwrap(),
        if own { before } else { after }
    );
}

/// Error-result painting uses the original native highlighter and matches the native factory.
fn builtin_error_result(
    binding: &ToolRendererBinding,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_, ToolRendererState>,
    text: &str,
) -> maestro_tui::tui::ComponentHandle {
    let output: Vec<_> = serde_json::from_value(json!([{"type":"text","text":text}])).unwrap();
    let result = maestro_extensions::ToolRenderResultLike {
        content: &output,
        details: None,
        terminate: None,
        is_error: Some(true),
    };
    let prepared = binding
        .render_result
        .as_ref()
        .unwrap()
        .prepare(result.with_details(result.details))
        .unwrap();
    let options = maestro_extensions::ToolRenderResultOptions {
        expanded: false,
        is_partial: true,
    };
    let component = prepared.render(options, theme, context).unwrap();
    let observed = component.try_render(60).unwrap();
    let native = maestro_extensions::local::create_read_tool_definition::<()>(
        "/".into(),
        maestro_extensions::ReadToolOptions::default(),
    );
    let native = ToolRendererBinding::from_raw(&native)
        .unwrap()
        .render_result
        .unwrap();
    let mut independent = context.with_state(context.state.clone());
    independent.last_component = None;
    let expected = native
        .prepare(result.with_details(result.details))
        .unwrap()
        .render(options, theme, &independent)
        .unwrap()
        .try_render(60)
        .unwrap();
    assert_eq!(
        observed, expected,
        "native environment and error-result painting survive delegation"
    );
    component
}

/// A caller cannot invent native rendering effects by omitting the lent row.
#[test]
fn builtin_renderer_without_native_row_fails_clearly() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(drop),
            Rc::new(|_| {}),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let register = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        register(
            "builtin-register-renderer missing".into(),
            runner.create_command_context(),
        )
        .await
        .unwrap();
        let tool = runner
            .get_all_registered_tools()
            .into_iter()
            .find(|tool| tool.definition.name() == "builtin-render")
            .unwrap();
        let call = tool
            .definition
            .renderer_binding()
            .unwrap()
            .render_call
            .unwrap();
        let rows = maestro_extensions::ToolRendererState::default();
        let redraws = Rc::new(std::cell::Cell::new(0));
        let args = json!({"path":"first.txt"});
        let context = row_context(&args, &rows, &redraws);
        let result = call(context.args, &builtin_theme(), &context);
        assert_eq!(
            result.err().unwrap().message,
            "native tool renderer requires a native row"
        );
    });
}

/// A complete theme for the native read paint paths under test.
fn builtin_theme() -> maestro_theme::LiveTheme {
    let keys = [
        maestro_theme::ThemeColor::Error,
        maestro_theme::ThemeColor::ToolTitle,
        maestro_theme::ThemeColor::Accent,
        maestro_theme::ThemeColor::Warning,
        maestro_theme::ThemeColor::Dim,
        maestro_theme::ThemeColor::ToolOutput,
    ];
    let colors = keys
        .into_iter()
        .map(|key| (key, maestro_theme::ColorValue::Index(7)));
    maestro_theme::LiveTheme::from_theme(Rc::new(
        maestro_theme::Theme::new(
            colors,
            [],
            maestro_theme::ColorMode::Color256,
            maestro_theme::ThemeOptions::default(),
        )
        .unwrap(),
    ))
}

/// A wrapper may change public observations without replacing native row effects.
#[test]
fn builtin_renderer_observes_guest_context_edits() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(drop),
            Rc::new(|_| {}),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let register = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        register(
            "builtin-register-renderer expanded".into(),
            runner.create_command_context(),
        )
        .await
        .unwrap();
        let tool = runner
            .get_all_registered_tools()
            .into_iter()
            .find(|tool| tool.definition.name() == "builtin-render")
            .unwrap();
        let call = tool
            .definition
            .renderer_binding()
            .unwrap()
            .render_call
            .unwrap();
        let rows = maestro_extensions::ToolRendererState::default();
        let redraws = Rc::new(std::cell::Cell::new(0));
        let args = json!({"path":"/work/docs/guide.md"});
        let mut context = row_context(&args, &rows, &redraws);
        context.expanded = false;
        Rc::get_mut(&mut context.environment).unwrap().readme_path = "/work/README.md".into();
        let theme = builtin_theme();
        let observed = call(context.args, &theme, &context)
            .unwrap()
            .try_render(60)
            .unwrap();
        let native = maestro_extensions::local::create_read_tool_definition::<()>(
            "/".into(),
            maestro_extensions::ReadToolOptions::default(),
        );
        let mut expanded = context.with_state(rows.state::<Value>());
        expanded.expanded = true;
        let expected = native.render_call.unwrap()(context.args, &theme, &expanded)
            .unwrap()
            .try_render(60)
            .unwrap();
        assert_eq!(observed, expected);
        assert_eq!(redraws.get(), 1);
        bus.clear();
    });
}

/// The original native rendering effect calls back into the same component instance.
struct ReenterHighlight(maestro_extensions::EventBus<maestro_extensions::Local>);
impl maestro_theme::SyntaxHighlighter for ReenterHighlight {
    fn supports_language(&self, language: &str) -> bool {
        language == "rust"
    }
    fn highlight(
        &self,
        code: &str,
        _: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        drop(
            self.0
                .emit("renderer-created", json!(["native-highlight", false])),
        );
        Ok(vec![maestro_theme::SyntaxSpan {
            range: 0..code.len(),
            color: None,
        }])
    }
}

/// Collects values emitted by rendering and by guest listeners reentered from native effects.
fn render_observations(
    bus: &maestro_extensions::EventBusController<Local>,
    channel: &str,
) -> Rc<RefCell<Vec<Value>>> {
    let observations = Rc::new(RefCell::new(Vec::new()));
    let received = observations.clone();
    bus.on(
        channel,
        Rc::new(move |value, _| {
            received.borrow_mut().push((*value).clone());
            Ok(None)
        }),
    );
    observations
}

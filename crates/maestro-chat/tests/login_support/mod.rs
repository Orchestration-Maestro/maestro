//! Public dialog construction, operations and lossless corpus comparison.
mod records;
use crate::bindings;
use maestro_chat::{
    LoginDialogComponent, LoginDialogOptions, LoginDialogPlatform, LoginInputFuture,
};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::{Component, Container, Focusable, Input, Text, tui::InputHandler};
use records::{Case, Frame, Operation, Query};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};

/// Recorded six-role palette, independent of the shipped token pack.
pub fn colors(shift: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, ColorValue::Index(31 + shift)),
                (ThemeColor::Accent, ColorValue::Index(32 + shift)),
                (ThemeColor::Text, ColorValue::Index(33 + shift)),
                (ThemeColor::Muted, ColorValue::Index(34 + shift)),
                (ThemeColor::Dim, ColorValue::Index(35 + shift)),
                (ThemeColor::Warning, ColorValue::Index(36 + shift)),
            ],
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Map the source's host observation without consulting global environment.
fn platform(value: Option<&str>) -> LoginDialogPlatform {
    match value {
        Some("darwin") => LoginDialogPlatform::MacOs,
        Some("win32") => LoginDialogPlatform::Windows,
        _ => LoginDialogPlatform::Other,
    }
}
/// Completion observes abort before notifying and can reenter the panel.
fn completion(
    slot: Rc<RefCell<Weak<LoginDialogComponent>>>,
    events: Rc<RefCell<Vec<Vec<Value>>>>,
    reenter: bool,
) -> super::login_support::Completion {
    Rc::new(move |success, message: Option<&str>| {
        let dialog = slot.borrow().upgrade().unwrap();
        assert!(
            dialog.signal().is_aborted(),
            "signal must abort before completion"
        );
        events.borrow_mut().push(vec![Value::from("abort")]);
        events.borrow_mut().push(vec![
            Value::from("complete"),
            Value::from(success),
            Value::from(message),
        ]);
        if reenter {
            dialog.show_progress("callback progress").unwrap();
            dialog.handle_input("\x1b");
        }
    })
}
/// Caller completion handle.
type Completion = Rc<dyn Fn(bool, Option<&str>)>;
/// Caller browser effect.
type Launcher = Rc<dyn Fn(&str)>;
/// All mutable fixture state belongs to this admitted transaction.
struct Run {
    /// Real retained dialog.
    dialog: Rc<LoginDialogComponent>,
    /// Retained content alias, independent of outer replacement.
    content: Rc<dyn Component>,
    /// Retained input alias, independent of visibility.
    input: Rc<dyn Component>,
    /// Original inherited child array.
    outer: maestro_tui::tui::ChildArray,
    /// Theme publication owner.
    state: maestro_theme::ThemeState,
    /// Ordered caller notifications.
    events: Rc<RefCell<Vec<Vec<Value>>>>,
    /// Caller render count.
    renders: Rc<Cell<usize>>,
    /// Browser launch operands.
    launches: Rc<RefCell<Vec<String>>>,
    /// Eager input receivers with originating operation identity.
    pending: Vec<(usize, LoginInputFuture)>,
    /// Settled observations, in order.
    outcomes: Vec<(usize, String, String)>,
}
impl Run {
    /// Build a controlled panel and obtain retained aliases through public children.
    fn new(query: &mut Query) -> Self {
        let (state, theme) = super::published::published(&colors(0));
        let events = Rc::new(RefCell::new(Vec::new()));
        let renders = Rc::new(Cell::new(0));
        let launches = Rc::new(RefCell::new(Vec::new()));
        let slot: Rc<RefCell<Weak<LoginDialogComponent>>> = Rc::default();
        let on_complete = completion(slot.clone(), events.clone(), query.reenter);
        let registry = query.providers.take().unwrap_or_else(|| {
            vec![records::Provider {
                id: "provider".into(),
                name: "Provider Display".into(),
            }]
        });
        let count = renders.clone();
        let launch = launches.clone();
        let dialog = Rc::new(
            LoginDialogComponent::new(
                Rc::new(move || count.set(count.get() + 1)),
                query.provider_id.as_deref().unwrap_or("provider"),
                on_complete,
                LoginDialogOptions {
                    theme,
                    provider_name_override: query.provider_name_override.take(),
                    title_override: query.title_override.take(),
                    platform: platform(query.platform.as_deref()),
                    provider_name_lookup: Some(Rc::new(move |id| {
                        registry
                            .iter()
                            .find(|provider| provider.id == id)
                            .map(|provider| provider.name.clone())
                    })),
                    launch_browser: Some(Rc::new(move |url| launch.borrow_mut().push(url.into()))),
                },
            )
            .unwrap(),
        );
        *slot.borrow_mut() = Rc::downgrade(&dialog);
        let outer = dialog.children();
        let content = outer.borrow()[2].clone();
        // Obtain the always-retained input through the public composition interface.
        drop(dialog.show_manual_input("").unwrap());
        let input = container(&content).children().borrow()[2].clone();
        container(&content).clear();
        renders.set(0);
        Self {
            dialog,
            content,
            input,
            outer,
            state,
            events,
            renders,
            launches,
            pending: Vec::new(),
            outcomes: Vec::new(),
        }
    }
    /// Read the retained typed input through its public component handle.
    fn input(&self) -> &Input {
        (self.input.as_ref() as &dyn std::any::Any)
            .downcast_ref()
            .unwrap()
    }
    /// Apply one attributed public operation.
    fn apply(&mut self, index: usize, operation: Operation) {
        match operation {
            Operation::Key(key) => self.dialog.handle_input(&key),
            Operation::Auth { auth, instructions } => self
                .dialog
                .show_auth(&auth, instructions.as_deref())
                .unwrap(),
            Operation::Manual { manual } => self
                .pending
                .push((index, self.dialog.show_manual_input(&manual).unwrap())),
            Operation::Prompt {
                prompt,
                placeholder,
            } => self.pending.push((
                index,
                self.dialog
                    .show_prompt(&prompt, placeholder.as_deref())
                    .unwrap(),
            )),
            Operation::Info { info } => self.dialog.show_info(&info).unwrap(),
            Operation::Waiting { waiting } => self.dialog.show_waiting(&waiting).unwrap(),
            Operation::Progress { progress } => self.dialog.show_progress(&progress).unwrap(),
            Operation::Value { value } => self.input().set_value(value),
            Operation::Child { input } => self.input().handle_input(&input),
            Operation::Focus { focus } => self.dialog.focus_flag().set(focus),
            Operation::InputFocus { focus } => self.input().focus_flag().set(focus),
            Operation::Theme { theme } => self.state.set_theme_instance(colors(theme)).unwrap(),
            Operation::Bindings {
                bindings: overrides,
            } => bindings::replace_toolkit(&overrides),
            Operation::Invalidate { invalidate } => {
                assert!(invalidate);
                self.dialog.invalidate();
            }
            Operation::ClearOuter { clear } => {
                assert!(clear);
                self.dialog.clear();
            }
            Operation::RestoreOuter { restore } => {
                assert!(restore);
                self.dialog.set_children(self.outer.clone());
            }
            Operation::ClearContent { clear } => {
                assert!(clear);
                container(&self.content).clear();
            }
            Operation::Append { append } => {
                container(&self.content).add_child(Rc::new(Text::new(append, 0, 0, None)));
            }
        }
        self.settle();
    }
    /// Poll each owned observer after the public operation, preserving origin order.
    fn settle(&mut self) {
        let mut context = Context::from_waker(Waker::noop());
        let mut waiting = Vec::new();
        for (origin, mut future) in self.pending.drain(..) {
            match future.as_mut().poll(&mut context) {
                Poll::Pending => waiting.push((origin, future)),
                Poll::Ready(Ok(value)) => self.outcomes.push((origin, "ok".into(), value)),
                Poll::Ready(Err(error)) => {
                    self.outcomes
                        .push((origin, "error".into(), error.to_string()));
                }
            }
        }
        self.pending = waiting;
    }
    /// Keep every recorded distinction; only source-unobserved fields stay absent.
    fn frame(&self, name: &str, width: usize) -> Frame {
        Frame {
            lines: self.dialog.render(width),
            renders: self.renders.get(),
            launch_urls: self.launches.borrow().clone(),
            events: self.events.borrow().clone(),
            outcomes: self.outcomes.clone(),
            aborted: self.dialog.signal().is_aborted(),
            value: (name.contains("input") || name.contains("focus") || name.contains("prompt"))
                .then(|| self.input().get_value()),
            focus: (name == "login_focus_forwards_to_retained_input").then(|| {
                [
                    self.dialog.focus_flag().get(),
                    self.input().focus_flag().get(),
                ]
            }),
        }
    }
}
/// Downcast an inherited content handle without replacing its identity.
fn container(component: &Rc<dyn Component>) -> &Container {
    (component.as_ref() as &dyn std::any::Any)
        .downcast_ref()
        .unwrap()
}
/// Every mandatory corpus group has one public witness.
const OWNERS: [&str; 21] = [
    "login_title_uses_registry_and_override_precedence",
    "authorization_rows_keep_links_hints_and_instructions",
    "authorization_replaces_content_without_resetting_input",
    "manual_input_appends_and_starts_empty",
    "prompt_input_resets_freezes_and_submits_exact_text",
    "sequential_prompts_freeze_submitted_rows_and_share_the_input",
    "information_replaces_rows_and_keeps_caller_text",
    "information_keeps_hidden_pending_input",
    "waiting_appends_spacer_message_and_cancel_hint",
    "progress_appends_only_message_and_requests_render",
    "pending_input_rejects_after_signal_abort",
    "completion_reports_cancellation_once",
    "login_callbacks_allow_reentry_without_cycles",
    "login_child_cancel_uses_same_completion",
    "login_bindings_route_cancel_before_input_submission",
    "login_child_submission_uses_retained_input",
    "login_focus_forwards_to_retained_input",
    "login_theme_capture_and_live_borders_are_distinct",
    "login_container_edits_preserve_retained_content",
    "login_wrapping_and_escapes_use_existing_text",
    "browser_launcher_receives_literal_url_without_shell_expansion",
];

/// Replay all queries owned by this witness, asserting every boundary exactly.
pub fn replay(name: &str) {
    let _guard = super::keybindings::bindings_guard();
    let source = include_str!("../fixtures/login_dialog.json");
    let raw: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    let mut unique = std::collections::HashSet::new();
    for row in &raw {
        assert!(
            unique.insert((row["test"].clone().to_string(), row["input"].to_string())),
            "duplicate query"
        );
    }
    let cases: Vec<Case> = serde_json::from_str(source).unwrap();
    assert!(
        cases
            .iter()
            .all(|case| OWNERS.contains(&case.test.as_str())),
        "orphan group"
    );
    assert!(
        OWNERS
            .iter()
            .all(|owner| cases.iter().any(|case| case.test == *owner)),
        "missing group"
    );
    let mut count = 0;
    for mut case in cases.into_iter().filter(|case| case.test == name) {
        count += 1;
        bindings::replace_toolkit(&case.input.bindings.take().unwrap_or_default());
        let width = case.input.width.unwrap_or(80);
        let mut run = Run::new(&mut case.input);
        let mut expected = case.expected.frames.into_iter();
        assert_eq!(
            run.frame(name, width),
            expected.next().unwrap(),
            "initial frame"
        );
        for (index, operation) in case.input.ops.into_iter().enumerate() {
            run.apply(index, operation);
            assert_eq!(
                run.frame(name, width),
                expected.next().unwrap(),
                "operation {index}"
            );
        }
        assert!(expected.next().is_none(), "unread recorded frames");
    }
    assert!(count > 0, "missing corpus group");
}
#[cfg(unix)]
pub mod native;
/// Controlled default-registry panel with selectable launcher.
pub fn controlled(
    platform: LoginDialogPlatform,
    launch_browser: Option<Launcher>,
) -> (LoginDialogComponent, Rc<Cell<usize>>) {
    let (_, theme) = super::published::published(&colors(0));
    let renders = Rc::new(Cell::new(0));
    let count = renders.clone();
    let dialog = LoginDialogComponent::new(
        Rc::new(move || count.set(count.get() + 1)),
        "provider",
        Rc::new(|_, _| {}),
        LoginDialogOptions {
            theme,
            provider_name_override: None,
            title_override: None,
            platform,
            provider_name_lookup: None,
            launch_browser,
        },
    )
    .unwrap();
    (dialog, renders)
}
/// Retained input alias obtained from an appended public input row.
pub fn retained_input(dialog: &LoginDialogComponent) -> Rc<dyn Component> {
    let content = dialog.children().borrow()[2].clone();
    let children = container(&content).children();
    children
        .borrow()
        .iter()
        .find(|child| (child.as_ref() as &dyn std::any::Any).is::<Input>())
        .unwrap()
        .clone()
}
/// Completion must observe both abort and rejected input; later cancellation rejects again silently.
pub fn cancellation_order() {
    let (_, theme) = super::published::published(&colors(0));
    let observer: Rc<RefCell<Option<LoginInputFuture>>> = Rc::default();
    let signal: Rc<RefCell<Option<maestro_tui::Cancellation>>> = Rc::default();
    let completed = Rc::new(Cell::new(0));
    let (input, aborted, count) = (observer.clone(), signal.clone(), completed.clone());
    let complete = Rc::new(move |_, _: Option<&str>| {
        assert!(aborted.borrow().as_ref().unwrap().is_aborted());
        let mut pending = input.borrow_mut().take().unwrap();
        assert_eq!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(Err(maestro_chat::LoginDialogError::Cancelled))
        );
        count.set(count.get() + 1);
    });
    let dialog = LoginDialogComponent::new(
        Rc::new(|| {}),
        "p",
        complete,
        LoginDialogOptions {
            theme,
            provider_name_override: None,
            title_override: None,
            platform: LoginDialogPlatform::Other,
            provider_name_lookup: None,
            launch_browser: Some(Rc::new(|_| {})),
        },
    )
    .unwrap();
    *signal.borrow_mut() = Some(dialog.signal().clone());
    *observer.borrow_mut() = Some(dialog.show_prompt("Cancellation boundary", None).unwrap());
    dialog.handle_input("\x1b");
    let mut later = dialog.show_manual_input("Subsequent entry").unwrap();
    dialog.handle_input("\x1b");
    assert_eq!(
        later.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(maestro_chat::LoginDialogError::Cancelled))
    );
    assert_eq!(completed.get(), 1);
}

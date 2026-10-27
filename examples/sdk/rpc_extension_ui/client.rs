//! Retained terminal widgets and correlated RPC conversation state.
use super::native::Action;
use super::output_log::OutputLog;
use maestro_app::rpc_types::{
    RpcCommand, RpcCommandBody, RpcExtensionUIMethod as Method, RpcExtensionUIRequest,
    RpcExtensionUIResponse, RpcResponse, RpcSessionCommand,
};
use maestro_app::{configuration::config, is_whitespace_scalar, serialize_json_line};
use maestro_theme::{LiveTheme, ThemeState};
use maestro_tui::tui::ComponentHandle;
use maestro_tui::{
    Container, Editor, EditorOptions, Input, SelectItem, SelectList, SelectListLayoutOptions, TUI,
    Text,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Write},
    process::{Child, ChildStdin, ExitStatus},
    rc::Rc,
    time::Duration,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// The dialog's original request distinguishes remote answers from local demonstrations.
struct Dialog {
    /// The shared protocol record, never a second wire schema.
    request: RpcExtensionUIRequest,
    /// Whether its answer must go back to the child.
    remote: bool,
}
/// The terminal widgets and correlation state of one child conversation.
pub(super) struct Client {
    /// Retained frame writer.
    tui: TUI,
    /// Supplied brand-backed styles.
    theme: LiveTheme,
    /// Streamed output and notices.
    log: Rc<OutputLog>,
    /// Ordinary user input, retained while a dialog replaces it.
    prompt: Rc<Input>,
    /// The current prompt or dialog chrome.
    bottom: ComponentHandle,
    /// The current input target.
    focus: ComponentHandle,
    /// Working indicator text.
    working: Rc<Text>,
    /// Whether agent events describe an active run.
    streaming: bool,
    /// Whether this run has begun its text output.
    has_text: bool,
    /// The current number of working dots.
    dots: usize,
    /// Live dialog and its correlation identifier.
    active: Option<Dialog>,
    /// Concurrent dialog requests wait rather than overwriting another answer.
    dialogs: VecDeque<Dialog>,
    /// Callback delivery to the owning event loop.
    actions: UnboundedSender<Action>,
    /// The sole command and UI-reply writer.
    input: ChildStdin,
    /// Commands awaiting their correlated response.
    pending: BTreeMap<String, String>,
    /// Monotonic command identifier suffix.
    serial: u64,
}
impl Client {
    /// Prepares retained widgets without entering raw-terminal mode.
    pub(super) fn new(
        tui: TUI,
        input: ChildStdin,
        actions: UnboundedSender<Action>,
    ) -> io::Result<Self> {
        let themes = Rc::new(ThemeState::new(config::get_theme_directories()?));
        themes.init_theme(None, None).map_err(io::Error::other)?;
        let prompt = Rc::new(Input::new());
        let sender = actions.clone();
        prompt.set_on_submit(Some(Rc::new(move |text| {
            let _ = sender.send(Action::Submit(text.into()));
        })));
        let bottom = Rc::new(Container::new());
        bottom.add_child(Rc::new(Text::new("You:".into(), 0, 0, None)));
        bottom.add_child(prompt.clone());
        Ok(Self {
            tui,
            theme: themes.theme(),
            log: Rc::default(),
            focus: prompt.clone(),
            prompt,
            bottom,
            working: Rc::new(Text::new("Agent: Working.".into(), 0, 0, None)),
            streaming: false,
            has_text: false,
            dots: 1,
            active: None,
            dialogs: VecDeque::new(),
            actions,
            input,
            pending: BTreeMap::new(),
            serial: 0,
        })
    }
    /// Writes only through the application's JSONL owner.
    fn write(&mut self, record: &Value) -> io::Result<()> {
        self.input
            .write_all(serialize_json_line(record)?.as_bytes())?;
        self.input.flush()
    }
    /// Sends a typed command and retains the identifier that its response must repeat.
    fn command(&mut self, command: RpcSessionCommand) -> io::Result<()> {
        self.serial += 1;
        let id = format!("req_{}", self.serial);
        let record = serde_json::to_value(RpcCommand {
            id: Some(id.clone()),
            command: RpcCommandBody::Session(command),
        })?;
        let kind = record["type"]
            .as_str()
            .ok_or_else(|| io::Error::other("Command has no type"))?
            .to_owned();
        self.write(&record)?;
        self.pending.insert(id, kind);
        Ok(())
    }
    /// Restores the ordinary input without losing text prefilled during a dialog.
    fn show_prompt(&mut self) {
        let bottom = Rc::new(Container::new());
        bottom.add_child(Rc::new(Text::new("You:".into(), 0, 0, None)));
        bottom.add_child(self.prompt.clone());
        self.bottom = bottom;
        self.focus = self.prompt.clone();
    }
    /// Rebuilds the retained frame around its active input.
    fn redraw(&self) {
        self.log
            .visible
            .set(self.tui.terminal().borrow().rows().saturating_sub(8).max(1));
        self.tui.clear();
        self.tui.add_child(self.log.clone());
        if self.streaming {
            self.tui.add_child(self.working.clone());
        }
        self.tui.add_child(self.bottom.clone());
        self.tui.set_focus(Some(self.focus.clone()));
        self.tui.request_render(false);
    }
    /// Correlates a response before showing the child's refusal, if any.
    fn response(&mut self, record: Value) -> io::Result<()> {
        let response: RpcResponse = serde_json::from_value(record)?;
        let (id, command, error) = match response {
            RpcResponse::Success { id, command, .. } => (id, Some(command), None),
            RpcResponse::Error { id, command, error } => (id, command, Some(error)),
            RpcResponse::Workspace(_) => {
                return Err(io::Error::other("Unexpected workspace response"));
            }
        };
        let expected = id.and_then(|id| self.pending.remove(&id));
        if expected.is_none() || expected != command {
            return Err(io::Error::other("Uncorrelated RPC response"));
        }
        if let Some(error) = error {
            self.log.append(format!("[error] {error}"));
        }
        Ok(())
    }
    /// Projects streamed records without retaining a second copy of protocol types.
    fn record(&mut self, record: Value) -> io::Result<()> {
        match record["type"].as_str() {
            Some("response") => self.response(record)?,
            Some("extension_ui_request") => self.ui(serde_json::from_value(record)?, true)?,
            Some("agent_start") => {
                self.streaming = true;
                self.has_text = false;
                self.dots = 1;
            }
            Some("agent_end") => {
                self.streaming = false;
                self.log.append("");
            }
            Some("message_update") => self.message_update(&record["assistantMessageEvent"]),
            Some("tool_execution_start") => self.log.append(format!(
                "[tool: {}]",
                record["toolName"].as_str().unwrap_or_default()
            )),
            Some("tool_execution_end") => self.log.append(format!(
                "[result: {}...]",
                record["result"]
                    .to_string()
                    .chars()
                    .take(120)
                    .collect::<String>()
            )),
            _ => {}
        }
        Ok(())
    }
    /// Starts a text block once, then appends only the supplied deltas.
    fn message_update(&mut self, event: &Value) {
        if event["type"] != "text_delta" {
            return;
        }
        if !self.has_text {
            self.log.append("");
            self.log.append("Agent:");
            self.has_text = true;
        }
        if let Some(delta) = event["delta"].as_str() {
            self.log.append_raw(delta);
        }
    }

    /// Displays notices immediately and queues only methods that require an answer.
    fn ui(&mut self, request: RpcExtensionUIRequest, remote: bool) -> io::Result<()> {
        match &request.method {
            Method::Notify { message, .. } => self.log.append(format!("Notification: {message}")),
            Method::SetStatus {
                status_key,
                status_text,
            } => self.log.append(format!(
                "Notification: [status: {status_key}] {}",
                status_text.as_deref().unwrap_or("(cleared)")
            )),
            Method::SetWidget {
                widget_key,
                widget_lines: Some(lines),
                ..
            } if !lines.is_empty() => {
                self.log
                    .append(format!("Notification: [widget: {widget_key}]"));
                for line in lines {
                    self.log.append(format!("  {line}"));
                }
            }
            Method::SetWidget { .. } => {}
            Method::SetTitle { title } => self.tui.terminal().borrow_mut().set_title(title)?,
            Method::SetEditorText { text } => self.prompt.set_value(text.clone()),
            _ => {
                self.dialogs.push_back(Dialog { request, remote });
                self.next_dialog()?;
            }
        }
        Ok(())
    }
    /// Produces a callback repeating the exact active request identifier.
    fn answer(&self, confirmed: bool) -> Rc<dyn Fn(&str)> {
        let sender = self.actions.clone();
        let id = self
            .active
            .as_ref()
            .map(|dialog| dialog.request.id.clone())
            .unwrap_or_default();
        Rc::new(move |value| {
            let value = value.trim_matches(is_whitespace_scalar);
            let response = RpcExtensionUIResponse {
                id: id.clone(),
                value: (!confirmed && !value.is_empty()).then(|| value.into()),
                confirmed: confirmed.then_some(value == "Yes"),
                cancelled: (!confirmed && value.is_empty()).then_some(true),
            };
            let _ = sender.send(Action::Answer(response));
        })
    }
    /// Composes one dialog with existing toolkit editors and selectors.
    fn next_dialog(&mut self) -> io::Result<()> {
        if self.active.is_some() {
            return Ok(());
        }
        let Some(dialog) = self.dialogs.pop_front() else {
            self.show_prompt();
            return Ok(());
        };
        let method = dialog.request.method.clone();
        self.active = Some(dialog);
        let confirm = matches!(method, Method::Confirm { .. });
        let answer = self.answer(confirm);
        let cancel_answer = answer.clone();
        let cancel: Rc<dyn Fn()> = Rc::new(move || cancel_answer(""));
        let (title, body): (String, ComponentHandle) = match method {
            Method::Select { title, options, .. } => (title, self.select(options, answer, cancel)),
            Method::Confirm { title, message, .. } => (
                format!("{title}: {message}"),
                self.select(vec!["Yes".into(), "No".into()], answer, cancel),
            ),
            Method::Input {
                title, placeholder, ..
            } => {
                let input = Rc::new(Input::new());
                input.set_on_submit(Some(answer));
                input.set_on_escape(Some(cancel));
                (
                    placeholder.map_or(title.clone(), |hint| format!("{title} ({hint})")),
                    input,
                )
            }
            Method::Editor { title, prefill } => {
                let editor = Rc::new(Editor::new(
                    &self.tui,
                    maestro_theme::get_editor_theme(self.theme.clone()),
                    EditorOptions::default(),
                ));
                if let Some(text) = prefill {
                    editor.set_text(&text);
                }
                editor.set_on_submit(Some(answer));
                (title, editor)
            }
            _ => return Err(io::Error::other("Notice queued as a dialog")),
        };
        let bottom = Rc::new(Container::new());
        bottom.add_child(Rc::new(Text::new(title, 0, 0, None)));
        bottom.add_child(body.clone());
        bottom.add_child(Rc::new(Text::new(
            "Enter to submit, Esc to cancel".into(),
            0,
            0,
            None,
        )));
        self.bottom = bottom;
        self.focus = body;
        Ok(())
    }
    /// Keeps the authored eight-row selection viewport and original option values.
    fn select(
        &self,
        options: Vec<String>,
        answer: Rc<dyn Fn(&str)>,
        cancel: Rc<dyn Fn()>,
    ) -> ComponentHandle {
        let items = options
            .into_iter()
            .map(|value| SelectItem {
                label: value.clone(),
                value,
                description: None,
            })
            .collect();
        let list = Rc::new(SelectList::new(
            items,
            8,
            maestro_theme::get_select_list_theme(self.theme.clone()),
            SelectListLayoutOptions::default(),
        ));
        list.set_on_select(Some(Rc::new(move |item| answer(&item.value))));
        list.set_on_cancel(Some(cancel));
        list
    }
    /// Sends nonempty prompt input without losing its authored interior whitespace.
    fn submit(&mut self, text: &str) -> io::Result<()> {
        let text = text.trim_matches(is_whitespace_scalar);
        if text.is_empty() {
            return Ok(());
        }
        self.prompt.set_value(String::new());
        self.log.append(format!("You: {text}"));
        self.command(RpcSessionCommand::Prompt {
            message: text.into(),
            images: None,
            streaming_behavior: None,
        })
    }
    /// Settles the active dialog once before admitting the next queued request.
    fn reply(&mut self, response: RpcExtensionUIResponse) -> io::Result<()> {
        let Some(active) = self.active.take() else {
            return Ok(());
        };
        if active.request.id != response.id {
            self.active = Some(active);
            return Ok(());
        }
        if active.remote {
            self.write(&serde_json::to_value(response)?)?;
        } else {
            self.log.append(format!(
                "Notification: {}",
                response.value.unwrap_or_else(|| response
                    .confirmed
                    .map_or("Cancelled".into(), |value| format!("Confirmed: {value}")))
            ));
        }
        self.next_dialog()
    }
    /// Returns true only for an explicit exit, never for an active-run cancellation.
    fn action(&mut self, action: Action) -> io::Result<bool> {
        match action {
            Action::Record(record) => self.record(record)?,
            Action::Submit(text) => self.submit(&text)?,
            Action::Answer(response) => self.reply(response)?,
            Action::Escape if self.active.is_some() => (self.answer(
                self.active
                    .as_ref()
                    .is_some_and(|dialog| matches!(dialog.request.method, Method::Confirm { .. })),
            ))(""),
            Action::Escape if self.streaming => {
                self.command(RpcSessionCommand::Abort)?;
                self.log.append("[aborted]");
            }
            Action::Escape | Action::Quit => return Ok(true),
            Action::Failed(error) => return Err(error),
        }
        Ok(false)
    }
    /// Drives callbacks and the authored 400 ms indicator while observing child exit.
    pub(super) async fn run(
        &mut self,
        child: &mut Child,
        mut actions: UnboundedReceiver<Action>,
    ) -> io::Result<Option<ExitStatus>> {
        self.log.append("RPC Chat");
        self.log
            .append("Type a message and press Enter. Esc to abort or exit. Ctrl+D to quit.");
        self.redraw();
        let mut tick = tokio::time::interval(Duration::from_millis(400));
        loop {
            tokio::select! {
                Some(action) = actions.recv() => if self.action(action)? { return Ok(None); },
                _ = tick.tick() => {
                    if let Some(status) = child.try_wait()? { return Ok(Some(status)); }
                    self.dots = self.dots % 3 + 1;
                    self.working.set_text(format!("Agent: Working{}", ".".repeat(self.dots)));
                }
            }
            self.redraw();
        }
    }
}

//! The scenario driver for the controlled adapter.
use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentToolUpdateCallback, CallbackEmitter, ExtensionEvent, ExtensionEventResult,
    InputEventResult, SessionBeforeCompactEvent, SessionEvent, ToolCallEvent,
};

use super::controlled::{ControlledHost, Flag, Log};
use super::guest_family as fixtures;
use super::scenario::Driver;

/// Drives the author extension through the controlled host.
pub struct ControlledDriver {
    /// The controlled host the extension registered with.
    host: ControlledHost,
    /// The host's observations.
    log: Log,
    /// The signal of the previous compaction, which the extension retained.
    previous: Option<Flag>,
    /// The signal of the previous tool call, which the extension retained.
    tool_signal: Option<Flag>,
}

impl ControlledDriver {
    /// A driver with an empty log.
    pub fn new() -> Self {
        let log = Log::default();
        Self {
            host: ControlledHost::new(log.clone()),
            log,
            previous: None,
            tool_signal: None,
        }
    }
}

impl Driver for ControlledDriver {
    async fn start(&mut self) -> Result<(), String> {
        super::author::factory(self.host.api()).await
    }

    async fn tool(&mut self, call_id: &str) {
        let Some((prepare, execute)) = self.host.tool("echo") else {
            return;
        };
        let raw = serde_json::json!({ "text": "x" });
        let emitter = self.host.emitter();
        let prepared = match prepare {
            Some(prepare) => prepare(raw, CallbackEmitter::new(&emitter)),
            None => Ok(raw),
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(message) => return self.log.push(format!("prepare failed: {message}")),
        };
        self.log.push(format!("prepared {prepared}"));
        let flag = Flag::default();
        self.tool_signal = Some(flag.clone());
        let (log, call) = (self.log.clone(), call_id.to_owned());
        let update: AgentToolUpdateCallback = Rc::new(move |partial| {
            log.push(format!(
                "tool-update {call} {}",
                fixtures::describe(&partial.content)
            ));
            Ok(())
        });
        let outcome = execute(
            call_id.to_owned(),
            prepared,
            Some(flag.signal()),
            Some(update),
            ControlledHost::context(),
        )
        .await;
        self.log.push(match outcome {
            Ok(result) => format!(
                "tool-result {call_id} content={} details={} terminate={}",
                fixtures::describe(&result.content),
                result.details.unwrap_or_default(),
                result.terminate.unwrap_or_default()
            ),
            Err(message) => format!("tool-result {call_id} error={message}"),
        });
    }

    fn abort_tool_signal(&mut self) {
        self.tool_signal.iter().for_each(Flag::abort);
    }

    async fn input(&mut self, text: &str) {
        let mut event = ExtensionEvent::Input(fixtures::input(text));
        let decision = match self.host.dispatch("input", &mut event).await {
            Ok(Some(ExtensionEventResult::Input(InputEventResult::Transform(replacement)))) => {
                format!("transform:{}", replacement.text)
            }
            Ok(_) => "other".to_owned(),
            Err(message) => format!("error:{message}"),
        };
        self.log.push(format!("input {text} -> {decision}"));
    }

    async fn tool_call(&mut self, command: &str) {
        let mut event = ExtensionEvent::ToolCall(fixtures::bash_call(command));
        let decision = self.host.dispatch("tool_call", &mut event).await;
        let ExtensionEvent::ToolCall(ToolCallEvent::Bash(call)) = event else {
            return;
        };
        let error = decision.err().unwrap_or_default();
        self.log.push(format!(
            "tool_call {command} -> command={} error={error}",
            call.input.command
        ));
    }

    async fn compact(&mut self, aborted: bool) {
        let flag = Flag::default();
        if aborted {
            self.previous.iter().for_each(Flag::abort);
            flag.abort();
        }
        self.previous = Some(flag.clone());
        let mut event =
            ExtensionEvent::Session(SessionEvent::BeforeCompact(SessionBeforeCompactEvent {
                data: fixtures::compact_data(),
                signal: flag.signal(),
            }));
        let Ok(Some(ExtensionEventResult::SessionBeforeCompact(result))) = self
            .host
            .dispatch("session_before_compact", &mut event)
            .await
        else {
            return;
        };
        let summary = result
            .compaction
            .map(|compaction| compaction.summary)
            .unwrap_or_default();
        let cancel = result.cancel.unwrap_or_default();
        self.log.push(format!(
            "compact aborted={aborted} -> cancel={cancel} summary={summary}"
        ));
    }

    async fn command(&mut self, args: &str) {
        let (hold, started, open) = super::host::gate();
        self.host.hold_next_session(hold);
        let handler = self.host.command_handler("replace");
        let Some(handler) = handler else { return };
        let run = handler(args.to_owned(), self.host.command_context());
        let release = async {
            let _ = started.await;
            let _ = open.send(());
        };
        let (finished, ()) = super::join::both(run, release).await;
        if let Err(message) = finished {
            self.log.push(format!("command failed: {message}"));
        }
    }

    async fn release_all(&mut self) {
        self.host.release_all();
    }

    fn transcript(&self) -> Vec<String> {
        self.log.lines()
    }
}

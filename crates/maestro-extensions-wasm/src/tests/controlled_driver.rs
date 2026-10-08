//! Drives the shared extension through the controlled adapter.

use maestro_extensions_wasm::{
    ExtensionEvent, ExtensionEventResult, InputEventResult, SessionBeforeCompactEvent, SessionEvent,
};

use super::controlled::{ControlledHost, Flag, Log};
use super::guest_family as fixtures;
use super::scenario::Driver;

/// The controlled adapter with the state the scenario keeps between steps.
pub struct ControlledDriver {
    /// The host the extension registers with.
    host: ControlledHost,
    /// What the host observed.
    log: Log,
    /// The signal of the previous compaction, which the driver cancels before the next one.
    previous: Option<Flag>,
}

impl ControlledDriver {
    /// A driver with a fresh host.
    pub fn new() -> Self {
        let log = Log::default();
        Self {
            host: ControlledHost::new(log.clone()),
            log,
            previous: None,
        }
    }
}

impl Driver for ControlledDriver {
    async fn start(&mut self) -> Result<(), String> {
        super::author::factory(self.host.api()).await
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

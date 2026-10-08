//! Drives the shared extension through the controlled adapter, calling the same functions the
//! component calls.

use std::future::Future;

use super::controlled::{Controlled, Flag};
use super::guest_family as fixtures;
use super::observed::gate;
use super::scenario::Driver;
use crate::component_adapter::{Exports, release};
use crate::loader::{Extension, load_extension_from_factory};
use crate::types::{ExtensionAPI, ExtensionFuture};

/// The shared author extension, loaded from its factory.
struct Author;

impl Extension for Author {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        load_extension_from_factory(Box::new(super::author::factory), api)
    }
}

/// The controlled adapter with the state the scenario keeps between steps.
pub struct ControlledDriver {
    /// The host the extension registers with.
    host: Controlled,
    /// The signal of the previous compaction, which the driver cancels before the next one.
    previous: Option<Flag>,
}

impl ControlledDriver {
    /// A driver with a fresh host.
    pub fn new() -> Self {
        Self {
            host: Controlled::default(),
            previous: None,
        }
    }

    /// The component's exports served over the controlled host.
    fn exports(&self) -> Exports<Controlled> {
        Exports::new(self.host.clone())
    }

    /// The key a callback was registered under.
    fn handler(&self, name: &str) -> Result<u32, String> {
        self.host.observed().callback(name)
    }

    /// Releases the registration next in line; whether there was one.
    fn release_next(&self) -> bool {
        let next = self.host.observe().next_to_release();
        next.map(|(_, key)| release(key)).is_some()
    }

    /// Delivers an input to the handler registered as `name`.
    async fn deliver_input(
        &self,
        name: &str,
        text: &str,
    ) -> Result<fixtures::guest::InputOutcome, String> {
        let handler = self.handler(name)?;
        let ctx = self.host.context("/work");
        Ok(self
            .exports()
            .invoke_input(handler, fixtures::input(text), ctx)
            .await)
    }

    /// A cancellation flag for the handler that keeps its signal; cancelling it first
    /// cancels the signal that handler kept from the previous compaction.
    fn retained_flag(&mut self, aborted: bool) -> Flag {
        let flag = Flag::default();
        if aborted {
            self.previous.iter().for_each(Flag::abort);
            flag.abort();
        }
        self.previous = Some(flag.clone());
        flag
    }

    /// Delivers a before-compact event with `flag` to the handler registered as `name`.
    async fn deliver_compact(
        &self,
        name: &str,
        flag: &Flag,
    ) -> Result<fixtures::guest::SessionBeforeCompactOutcome, String> {
        let handler = self.handler(name)?;
        let ctx = self.host.context("/work");
        Ok(self
            .exports()
            .invoke_session_before_compact(
                handler,
                fixtures::compact_data(),
                self.host.signal(flag),
                ctx,
            )
            .await)
    }

    /// Runs the command with the wait for idle of its continuation held until the driver saw it
    /// pending.
    async fn run_command(&self, args: &str) -> Result<(), String> {
        let handler = self.handler("command replace")?;
        let ctx = self.host.command_context("/work");
        let (hold, started, open) = gate();
        self.host.observe().hold_next_idle(hold);
        let exports = self.exports();
        let run = exports.invoke_command(handler, args.to_owned(), ctx);
        let release = async {
            let _ = started.await;
            self.host.observe().suspended();
            let _ = open.send(());
        };
        super::join::alongside(run, release).await
    }
}

impl Drop for ControlledDriver {
    /// The callback table is shared by the thread, so a driver leaves nothing behind.
    fn drop(&mut self) {
        while self.release_next() {}
    }
}

impl Driver for ControlledDriver {
    async fn start(&mut self) -> Result<(), String> {
        self.exports().start::<Author>().await
    }

    async fn input(&mut self, text: &str) -> Result<String, String> {
        let outcome = self.deliver_input("event input", text).await?;
        Ok(fixtures::input_decision(&outcome.decision))
    }

    async fn compact(&mut self, aborted: bool) -> Result<String, String> {
        let flag = self.retained_flag(aborted);
        let outcome = self
            .deliver_compact("event session_before_compact", &flag)
            .await?;
        Ok(fixtures::compact_decision(&outcome.decision))
    }

    async fn trim_input(&mut self, text: &str) -> Result<String, String> {
        let outcome = self.deliver_input("event trim_input", text).await?;
        Ok(fixtures::trimmed(&outcome))
    }

    async fn note_compaction(&mut self, aborted: bool) -> Result<String, String> {
        let flag = Flag::default();
        if aborted {
            flag.abort();
        }
        let outcome = self.deliver_compact("event note_compaction", &flag).await?;
        Ok(fixtures::noted(&outcome))
    }

    async fn command(&mut self, args: &str) -> Result<(), String> {
        self.run_command(args).await
    }

    fn release_all(&mut self) -> impl Future<Output = ()> {
        while self.release_next() {}
        std::future::ready(())
    }

    fn reject_next_session(&mut self) {
        self.host.observe().reject_next_session();
    }

    async fn invoke_unknown_callbacks(&mut self) -> Vec<String> {
        let exports = self.exports();
        let decision = exports
            .invoke_input(4242, fixtures::input("x"), self.host.context("/work"))
            .await
            .decision;
        let continuation = exports
            .invoke_with_session(4242, self.host.replaced_context("/replacement"))
            .await;
        vec![
            decision.err().unwrap_or_default(),
            continuation.err().unwrap_or_default(),
        ]
    }

    fn log(&mut self, line: String) {
        self.host.observe().log(line);
    }

    fn transcript(&self) -> Vec<String> {
        self.host.observed().transcript.clone()
    }

    fn dropped(&self) -> Vec<u32> {
        self.host.observed().dropped.clone()
    }

    fn live(&self) -> usize {
        self.host.observed().live
    }
}

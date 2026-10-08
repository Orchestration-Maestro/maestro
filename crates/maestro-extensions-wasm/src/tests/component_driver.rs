//! Drives the built component through the test-only native host.

use std::path::PathBuf;

use wasmtime::component::Resource;

use super::host::{Harness, borrow, reported};
use super::host_family as fixtures;
use super::observed::{Observed, gate};
use super::scenario::Driver;

/// The component with the state the scenario keeps between steps.
pub struct ComponentDriver {
    /// Path of the built component.
    path: PathBuf,
    /// The running component, once started.
    harness: Option<Harness>,
    /// The signal of the previous compaction, which the driver cancels before the next one.
    previous: Option<u32>,
}

/// The running harness, or an error when the component was not started.
fn started(harness: &mut Option<Harness>) -> wasmtime::Result<&mut Harness> {
    harness
        .as_mut()
        .ok_or_else(|| wasmtime::format_err!("the component was not started"))
}

impl ComponentDriver {
    /// A driver for the component at `path`.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            harness: None,
            previous: None,
        }
    }

    /// Reads what the host observed; nothing before the component started.
    fn observed<T: Default>(&self, read: impl FnOnce(&Observed) -> T) -> T {
        self.harness
            .as_ref()
            .map(|harness| read(&harness.store.data().observed))
            .unwrap_or_default()
    }

    /// Delivers an input to the handler registered as `name`.
    async fn deliver_input(
        &mut self,
        name: &str,
        text: &str,
    ) -> wasmtime::Result<fixtures::guest::InputOutcome> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback(name)?;
        let ctx = harness.ordinary("/work")?;
        let exports = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_input(accessor, borrow(handler), fixtures::input(text), ctx)
                    .await
            })
            .await?
    }

    /// A cancellation flag for the handler that keeps its signal; cancelling it first
    /// cancels the signal that handler kept from the previous compaction.
    fn retained_flag(&mut self, aborted: bool) -> wasmtime::Result<u32> {
        let harness = started(&mut self.harness)?;
        if aborted && let Some(previous) = self.previous {
            harness.abort(previous)?;
        }
        let key = harness.flag(aborted)?;
        self.previous = Some(key);
        Ok(key)
    }

    /// Delivers a before-compact event with the flag `key` to the handler registered as `name`.
    async fn deliver_compact(
        &mut self,
        name: &str,
        key: u32,
    ) -> wasmtime::Result<fixtures::guest::SessionBeforeCompactOutcome> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback(name)?;
        let ctx = harness.ordinary("/work")?;
        let signal = Resource::new_own(key);
        let exports = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_session_before_compact(
                        accessor,
                        borrow(handler),
                        fixtures::compact_data(),
                        signal,
                        ctx,
                    )
                    .await
            })
            .await?
    }

    /// Runs the command with the wait for idle of its continuation held until the driver saw it
    /// pending.
    async fn run_command(&mut self, args: &str) -> wasmtime::Result<()> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback("command replace")?;
        let ctx = harness.command_context("/work")?;
        let (hold, started, open) = gate();
        harness.observed().hold_next_idle(hold);
        let exports = harness.exports.clone();
        let finished = harness
            .store
            .run_concurrent(async |accessor| {
                let call =
                    exports.call_invoke_command(accessor, borrow(handler), args.to_owned(), ctx);
                let release = async {
                    let _ = started.await;
                    accessor.with(|mut access| access.get().observed.suspended());
                    let _ = open.send(());
                };
                super::join::alongside(call, release).await
            })
            .await??;
        reported(finished)
    }

    /// Invokes handlers and continuations under an identity the component never registered,
    /// and returns the errors it reports.
    async fn unknown_callbacks(&mut self) -> wasmtime::Result<Vec<String>> {
        let harness = started(&mut self.harness)?;
        let unknown = harness.identity(4242)?;
        let (ctx, replaced) = (
            harness.ordinary("/work")?,
            harness.replaced("/replacement")?,
        );
        let exports = harness.exports.clone();
        let (decision, continuation) = harness
            .store
            .run_concurrent(async |accessor| {
                let decision = exports
                    .call_invoke_input(accessor, borrow(unknown), fixtures::input("x"), ctx)
                    .await;
                let continuation = exports
                    .call_invoke_with_session(accessor, borrow(unknown), replaced)
                    .await;
                (decision, continuation)
            })
            .await?;
        let decision = decision?.decision.err().unwrap_or_default();
        let continuation = continuation?.err().unwrap_or_default();
        Ok(vec![decision, continuation])
    }
}

impl Driver for ComponentDriver {
    async fn start(&mut self) -> Result<(), String> {
        let harness = Harness::start(&self.path)
            .await
            .map_err(|error| error.to_string())?;
        self.harness = Some(harness);
        Ok(())
    }

    async fn input(&mut self, text: &str) -> Result<String, String> {
        let outcome = self
            .deliver_input("event input", text)
            .await
            .map_err(|error| error.to_string())?;
        Ok(fixtures::input_decision(&outcome.decision))
    }

    async fn compact(&mut self, aborted: bool) -> Result<String, String> {
        let key = self
            .retained_flag(aborted)
            .map_err(|error| error.to_string())?;
        let outcome = self
            .deliver_compact("event session_before_compact", key)
            .await
            .map_err(|error| error.to_string())?;
        Ok(fixtures::compact_decision(&outcome.decision))
    }

    async fn trim_input(&mut self, text: &str) -> Result<String, String> {
        let outcome = self
            .deliver_input("event trim_input", text)
            .await
            .map_err(|error| error.to_string())?;
        Ok(fixtures::trimmed(&outcome))
    }

    async fn note_compaction(&mut self, aborted: bool) -> Result<String, String> {
        let key = started(&mut self.harness)
            .and_then(|harness| harness.flag(aborted))
            .map_err(|error| error.to_string())?;
        let outcome = self
            .deliver_compact("event note_compaction", key)
            .await
            .map_err(|error| error.to_string())?;
        Ok(fixtures::noted(&outcome))
    }

    async fn command(&mut self, args: &str) -> Result<(), String> {
        self.run_command(args)
            .await
            .map_err(|error| error.to_string())
    }

    async fn release_all(&mut self) {
        let Ok(harness) = started(&mut self.harness) else {
            return;
        };
        loop {
            let next = harness.observed().next_to_release();
            let Some((name, key)) = next else { break };
            if let Err(error) = harness.release(key).await {
                harness
                    .observed()
                    .log(format!("release {name} failed: {error}"));
            }
        }
    }

    fn reject_next_session(&mut self) {
        if let Ok(harness) = started(&mut self.harness) {
            harness.observed().reject_next_session();
        }
    }

    async fn invoke_unknown_callbacks(&mut self) -> Vec<String> {
        match self.unknown_callbacks().await {
            Ok(errors) => errors,
            Err(error) => vec![format!("invocation failed: {error}")],
        }
    }

    fn log(&mut self, line: String) {
        if let Ok(harness) = started(&mut self.harness) {
            harness.observed().log(line);
        }
    }

    fn transcript(&self) -> Vec<String> {
        self.observed(|observed| observed.transcript.clone())
    }

    fn dropped(&self) -> Vec<u32> {
        self.observed(|observed| observed.dropped.clone())
    }

    fn live(&self) -> usize {
        self.observed(|observed| observed.live)
    }
}

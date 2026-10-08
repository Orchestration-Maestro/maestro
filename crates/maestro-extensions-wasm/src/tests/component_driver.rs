//! Drives the built component through the test-only native host.

use std::path::PathBuf;

use wasmtime::component::Resource;

use super::host::{Harness, borrow, gate, reported};
use super::host_family::{self as fixtures, events};
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

    /// Delivers an input and describes what the handler decided.
    async fn deliver_input(&mut self, text: &str) -> wasmtime::Result<String> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback("event input")?;
        let ctx = harness.ordinary("/work")?;
        let exports = harness.exports.clone();
        let decision = harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_input(accessor, borrow(handler), fixtures::input(text), ctx)
                    .await
            })
            .await??;
        Ok(match decision {
            Ok(Some(events::ExtensionEventResult::Input(events::InputEventResult::Transform(
                replacement,
            )))) => format!("transform:{}", replacement.text),
            Ok(_) => "other".to_owned(),
            Err(message) => format!("error:{message}"),
        })
    }

    /// Delivers a before-compact event and describes what the handler decided.
    async fn deliver_compact(&mut self, aborted: bool) -> wasmtime::Result<String> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback("event session_before_compact")?;
        let ctx = harness.ordinary("/work")?;
        if aborted && let Some(previous) = self.previous {
            harness.abort(previous)?;
        }
        let key = harness.flag(aborted)?;
        self.previous = Some(key);
        let signal = Resource::new_own(key);
        let exports = harness.exports.clone();
        let decision = harness
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
            .await??;
        let Ok(Some(events::ExtensionEventResult::SessionBeforeCompact(result))) = decision else {
            return Err(wasmtime::format_err!("no compaction result: {decision:?}"));
        };
        let summary = result
            .compaction
            .map(|compaction| compaction.summary)
            .unwrap_or_default();
        Ok(format!(
            "cancel={} summary={summary}",
            result.cancel.unwrap_or_default()
        ))
    }

    /// Runs the command with its new session held until the driver saw it pending.
    async fn run_command(&mut self, args: &str) -> wasmtime::Result<()> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback("command replace")?;
        let ctx = harness.command_context("/work")?;
        let (hold, started, open) = gate();
        harness.hold_next_session(hold);
        let exports = harness.exports.clone();
        let finished = harness
            .store
            .run_concurrent(async |accessor| {
                let call =
                    exports.call_invoke_command(accessor, borrow(handler), args.to_owned(), ctx);
                let release = async {
                    started.await?;
                    let last = accessor.with(|mut access| access.get().transcript.last().cloned());
                    assert_eq!(last.as_deref(), Some("new-session start parent=parent"));
                    open.send(())
                        .map_err(|()| wasmtime::format_err!("the host stopped waiting"))
                };
                let (finished, released) = super::join::both(call, release).await;
                released.and(finished)
            })
            .await??;
        reported(finished)
    }

    /// Makes the next session operation fail without running its continuation.
    pub fn reject_next_session(&mut self) -> wasmtime::Result<()> {
        started(&mut self.harness)?.reject_next_session();
        Ok(())
    }

    /// Adds a line to the transcript when the component runs.
    fn note(&mut self, line: String) {
        if let Ok(harness) = started(&mut self.harness) {
            harness.note(line);
        }
    }

    /// The identities the component dropped, in drop order.
    pub fn dropped_identities(&mut self) -> wasmtime::Result<Vec<u32>> {
        Ok(started(&mut self.harness)?.store.data().dropped.clone())
    }

    /// Invokes handlers and continuations under an identity the component never registered,
    /// and returns the errors it reports.
    pub async fn invoke_unknown_callbacks(&mut self) -> wasmtime::Result<Vec<String>> {
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
        let decision = decision?.err().unwrap_or_default();
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

    async fn input(&mut self, text: &str) {
        let note = match self.deliver_input(text).await {
            Ok(decision) => format!("input {text} -> {decision}"),
            Err(error) => format!("input failed: {error}"),
        };
        self.note(note);
    }

    async fn compact(&mut self, aborted: bool) {
        let note = match self.deliver_compact(aborted).await {
            Ok(outcome) => format!("compact aborted={aborted} -> {outcome}"),
            Err(error) => format!("compact failed: {error}"),
        };
        self.note(note);
    }

    async fn command(&mut self, args: &str) {
        if let Err(error) = self.run_command(args).await {
            self.note(format!("command failed: {error}"));
        }
    }

    async fn release_all(&mut self) {
        let Ok(harness) = started(&mut self.harness) else {
            return;
        };
        for key in harness.store.data().order.clone() {
            if let Err(error) = harness.release(&key).await {
                harness.note(format!("release {key} failed: {error}"));
            }
        }
    }

    fn transcript(&self) -> Vec<String> {
        self.harness
            .as_ref()
            .map(|harness| harness.store.data().transcript.clone())
            .unwrap_or_default()
    }
}

//! The scenario driver for the built component.
use std::path::PathBuf;

use wasmtime::component::Resource;

use super::host::{Harness, borrow, gate, reported};
use super::host_family::{self as fixtures, events};
use super::scenario::Driver;

/// Drives the built author component through the test host.
pub struct ComponentDriver {
    /// Path of the built component.
    path: PathBuf,
    /// The instantiated component, once started.
    harness: Option<Harness>,
    /// Table key of the signal of the previous compaction, which the extension retained.
    previous: Option<u32>,
    /// Table key of the signal of the previous tool call, which the extension retained.
    tool_signal: Option<u32>,
}

/// The harness of a started component.
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
            tool_signal: None,
        }
    }

    /// Delivers an input event and returns the handler's decision.
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

    /// Prepares the echo tool's arguments, runs the tool and returns its result.
    async fn run_tool(&mut self, call_id: &str) -> wasmtime::Result<String> {
        let harness = started(&mut self.harness)?;
        let prepare = harness.callback("tool echo prepare")?;
        let execute = harness.callback("tool echo execute")?;
        let emitter = harness.emitter()?;
        let prepared = harness
            .exports
            .clone()
            .func_prepare_arguments()
            .call_async(
                &mut harness.store,
                (
                    borrow(prepare),
                    r#"{"text":"x"}"#,
                    Resource::new_borrow(emitter),
                ),
            )
            .await?
            .0;
        let prepared = reported(prepared)?;
        harness.note(format!("prepared {prepared}"));
        let ctx = harness.ordinary("/work")?;
        let update = harness.progress(call_id)?;
        let key = harness.flag(false)?;
        self.tool_signal = Some(key);
        let exports = harness.exports.clone();
        let (call, params) = (call_id.to_owned(), prepared.clone());
        let result = harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_tool(
                        accessor,
                        borrow(execute),
                        call,
                        params,
                        Some(Resource::new_own(key)),
                        Some(update),
                        ctx,
                    )
                    .await
            })
            .await??;
        let result = reported(result)?;
        let outcome = format!(
            "content={} details={} terminate={}",
            fixtures::describe(&result.content),
            result.details.unwrap_or_default(),
            result.terminate.unwrap_or_default()
        );
        Ok(outcome)
    }

    /// Delivers a bash tool call and returns the edited command and the failure.
    async fn deliver_tool_call(&mut self, command: &str) -> wasmtime::Result<String> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback("event tool_call")?;
        let ctx = harness.ordinary("/work")?;
        let exports = harness.exports.clone();
        let outcome = harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_tool_call(
                        accessor,
                        borrow(handler),
                        fixtures::bash_call(command),
                        ctx,
                    )
                    .await
            })
            .await??;
        let Some(events::ToolCallEvent::Bash(call)) = outcome.event else {
            return Err(wasmtime::format_err!(
                "the call did not come back as a bash call"
            ));
        };
        let error = outcome.decision.err().unwrap_or_default();
        Ok(format!("command={} error={error}", call.input.command))
    }

    /// Delivers a before-compact event and returns the cancel flag and summary.
    async fn deliver_compact(&mut self, aborted: bool) -> wasmtime::Result<String> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback("event session_before_compact")?;
        let ctx = harness.ordinary("/work")?;
        if aborted && let Some(previous) = self.previous {
            harness.abort(previous)?;
        }
        let key = harness.flag(aborted)?;
        self.previous = Some(key);
        let signal = wasmtime::component::Resource::new_own(key);
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

    /// Runs the replace command, holding its new session until the host saw it pending.
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
}

impl ComponentDriver {
    /// The identities the extension dropped, in drop order.
    ///
    /// # Errors
    /// Returns an error when the component was not started.
    pub fn dropped_identities(&mut self) -> wasmtime::Result<Vec<u32>> {
        Ok(started(&mut self.harness)?.store.data().dropped.clone())
    }

    /// Invokes an event handler and a continuation the extension never registered, and returns
    /// the messages it answers with.
    ///
    /// # Errors
    /// Returns an error when the component was not started or the engine fails.
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

    async fn tool(&mut self, call_id: &str) {
        match self.run_tool(call_id).await {
            Ok(outcome) => self.note(format!("tool-result {call_id} {outcome}")),
            Err(error) => self.note(format!("tool failed: {error}")),
        }
    }

    fn abort_tool_signal(&mut self) {
        let (Some(key), Ok(harness)) = (self.tool_signal, started(&mut self.harness)) else {
            return;
        };
        if let Err(error) = harness.abort(key) {
            harness.note(format!("abort failed: {error}"));
        }
    }

    async fn input(&mut self, text: &str) {
        let note = match self.deliver_input(text).await {
            Ok(decision) => format!("input {text} -> {decision}"),
            Err(error) => format!("input failed: {error}"),
        };
        self.note(note);
    }

    async fn tool_call(&mut self, command: &str) {
        let note = match self.deliver_tool_call(command).await {
            Ok(outcome) => format!("tool_call {command} -> {outcome}"),
            Err(error) => format!("tool_call failed: {error}"),
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

impl ComponentDriver {
    /// Records a line the driver observed.
    fn note(&mut self, line: String) {
        if let Ok(harness) = started(&mut self.harness) {
            harness.note(line);
        }
    }
}

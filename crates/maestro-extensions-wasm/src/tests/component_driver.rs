//! Drives the built component through the test-only native host.

use std::path::PathBuf;

use wasmtime::component::Resource;

use super::host::{Harness, borrow, release_held, reported};
use super::host_family as fixtures;
use super::observed::{Observed, gate};
use super::scenario::{Delivery, Driver, UNKNOWN};

/// The component with the state the scenario keeps between steps.
pub struct ComponentDriver {
    /// Path of the built component.
    path: PathBuf,
    /// The running component, once started.
    harness: Option<Harness>,
    /// The table keys of the flags lent so far, by handle.
    flags: Vec<u32>,
    /// The flag of the previous compaction, which the driver cancels before the next one.
    previous: Option<usize>,
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
            flags: Vec::new(),
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

    /// The table key of the flag lent under a handle.
    fn key(&self, signal: usize) -> wasmtime::Result<u32> {
        self.flags
            .get(signal)
            .copied()
            .ok_or_else(|| wasmtime::format_err!("no flag was lent as {signal}"))
    }

    /// The resources an event delivery lends: a context in `cwd` and the flag under `signal`.
    fn resources(
        &mut self,
        cwd: &str,
        signal: Option<usize>,
    ) -> wasmtime::Result<fixtures::guest::Capabilities> {
        let signal = signal
            .map(|signal| self.key(signal).map(Resource::new_own))
            .transpose()?;
        let ctx = started(&mut self.harness)?.ordinary(cwd)?;
        Ok(fixtures::guest::Capabilities { ctx, signal })
    }

    /// Delivers the event to the handler `handler`, optionally holding the next wait for idle
    /// pending until the driver saw it.
    async fn call(
        &mut self,
        handler: u32,
        event: &str,
        resources: fixtures::guest::Capabilities,
        hold: bool,
    ) -> wasmtime::Result<Result<fixtures::guest::EventOutcome, String>> {
        let harness = started(&mut self.harness)?;
        let (pause, pending, proceed) = gate();
        if hold {
            harness.observed().hold_next_idle(pause);
        }
        let exports = harness.exports.clone();
        let event = event.to_owned();
        harness
            .store
            .run_concurrent(async |accessor| {
                let call = exports.call_invoke_event(accessor, borrow(handler), event, resources);
                let release = release_held(accessor, hold.then_some((pending, proceed)));
                super::join::alongside(call, release).await
            })
            .await?
    }

    /// Runs the command registered as `name`; with `hold`, the wait for idle of its
    /// continuation stays pending until the driver saw it.
    async fn run_command(&mut self, name: &str, args: &str, hold: bool) -> wasmtime::Result<()> {
        let harness = started(&mut self.harness)?;
        let handler = harness.callback(&format!("command {name}"))?;
        let ctx = harness.command_context("/work")?;
        let (pause, pending, proceed) = gate();
        if hold {
            harness.observed().hold_next_idle(pause);
        }
        let exports = harness.exports.clone();
        let finished = harness
            .store
            .run_concurrent(async |accessor| {
                let call =
                    exports.call_invoke_command(accessor, borrow(handler), args.to_owned(), ctx);
                let release = release_held(accessor, hold.then_some((pending, proceed)));
                super::join::alongside(call, release).await
            })
            .await??;
        reported(finished)
    }

    /// Invokes a continuation under an identity the component never registered.
    async fn unknown_continuation(&mut self) -> wasmtime::Result<String> {
        let harness = started(&mut self.harness)?;
        let unknown = harness.identity(UNKNOWN)?;
        let replaced = harness.replaced("/replacement")?;
        let exports = harness.exports.clone();
        let continuation = harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_with_session(accessor, borrow(unknown), replaced)
                    .await
            })
            .await??;
        Ok(continuation.err().unwrap_or_default())
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

    fn identity(&self, name: &str) -> Result<u32, String> {
        self.harness
            .as_ref()
            .ok_or_else(|| "the component was not started".to_owned())?
            .callback(name)
            .map_err(|error| error.to_string())
    }

    async fn deliver(
        &mut self,
        handler: u32,
        event: &str,
        cwd: &str,
        signal: Option<usize>,
    ) -> Result<Delivery, String> {
        let resources = self
            .resources(cwd, signal)
            .map_err(|error| error.to_string())?;
        let outcome = self
            .call(handler, event, resources, false)
            .await
            .map_err(|error| error.to_string())??;
        Ok(fixtures::delivery(outcome))
    }

    async fn deliver_held(
        &mut self,
        handler: u32,
        event: &str,
        cwd: &str,
    ) -> Result<Delivery, String> {
        let resources = self
            .resources(cwd, None)
            .map_err(|error| error.to_string())?;
        let outcome = self
            .call(handler, event, resources, true)
            .await
            .map_err(|error| error.to_string())??;
        Ok(fixtures::delivery(outcome))
    }

    fn unregistered(&mut self) -> Result<u32, String> {
        started(&mut self.harness)
            .and_then(|harness| harness.identity(UNKNOWN))
            .map_err(|error| error.to_string())
    }

    fn lend_signal(&mut self, aborted: bool) -> Result<usize, String> {
        let key = started(&mut self.harness)
            .and_then(|harness| harness.flag(aborted))
            .map_err(|error| error.to_string())?;
        self.flags.push(key);
        Ok(self.flags.len() - 1)
    }

    fn abort_signal(&mut self, signal: usize) -> Result<(), String> {
        let key = self.key(signal).map_err(|error| error.to_string())?;
        started(&mut self.harness)
            .and_then(|harness| harness.abort(key))
            .map_err(|error| error.to_string())
    }

    fn previous_signal(&mut self) -> &mut Option<usize> {
        &mut self.previous
    }

    async fn prepare_key(&mut self, handler: u32, args: &str) -> Result<String, String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        harness
            .exports
            .func_invoke_prepare()
            .call_async(&mut harness.store, (borrow(handler), args))
            .await
            .map_err(|e| e.to_string())?
            .0
    }
    fn reborrow(&mut self, id: u32) -> Result<u32, String> {
        started(&mut self.harness)
            .and_then(|h| h.identity(id))
            .map_err(|e| e.to_string())
    }

    async fn tool(&mut self, run: super::scenario::ToolRun<'_>) -> Result<String, String> {
        let signal = run
            .signal
            .map(|s| self.key(s).map(Resource::new_own))
            .transpose()
            .map_err(|e| e.to_string())?;
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let handler = harness
            .callback(&format!("tool {}", run.name))
            .map_err(|e| e.to_string())?;
        let resources = fixtures::guest::ToolCapabilities {
            ctx: harness.ordinary("/work").map_err(|e| e.to_string())?,
            signal,
            update: if run.update {
                Some(harness.update().map_err(|e| e.to_string())?)
            } else {
                None
            },
        };
        let invocation = fixtures::guest::ToolInvocation {
            tool_call_id: run.id.into(),
            params: run.params.into(),
        };
        let exports = harness.exports.clone();
        let (hold, entered, open) = gate();
        if run.held {
            harness.observed().hold_next_idle(hold);
        }
        let flag = run.signal.map(|s| self.flags[s]);
        harness
            .store
            .run_concurrent(async |accessor| {
                let call =
                    exports.call_invoke_tool(accessor, borrow(handler), invocation, resources);
                let resume =
                    super::host::cancel_held(accessor, run.held.then_some((entered, open)), flag);
                super::join::alongside(call, resume).await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }

    fn tools(&self) -> Vec<String> {
        self.observed(|o| o.tools.clone())
    }
    fn updates(&self) -> Vec<String> {
        self.observed(|o| o.updates.clone())
    }

    async fn run_plain_command(&mut self, name: &str, args: &str) -> Result<(), String> {
        self.run_command(name, args, false)
            .await
            .map_err(|error| error.to_string())
    }

    async fn command(&mut self, args: &str) -> Result<(), String> {
        self.run_command("replace", args, true)
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

    async fn release(&mut self, name: &str) -> bool {
        let Ok(harness) = started(&mut self.harness) else {
            return false;
        };
        let key = harness.observed().take_registration(name);
        match key {
            Some(key) => harness.release(key).await.is_ok(),
            None => false,
        }
    }

    fn reject_next_session(&mut self) {
        if let Ok(harness) = started(&mut self.harness) {
            harness.observed().reject_next_session();
        }
    }

    async fn invoke_unknown_continuation(&mut self) -> String {
        match self.unknown_continuation().await {
            Ok(error) => error,
            Err(error) => format!("invocation failed: {error}"),
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

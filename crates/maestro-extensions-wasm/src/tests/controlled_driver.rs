//! Drives the shared extension through the controlled adapter, calling the same functions the
//! component calls.

use std::future::Future;

use super::controlled::{Controlled, Flag};
use super::guest_family as fixtures;
use super::observed::gate;
use super::scenario::{Delivery, Driver, UNKNOWN};
use crate::component_adapter::{Capabilities, Exports, release};
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
    /// The flags lent so far, by handle.
    flags: Vec<Flag>,
    /// The flag of the previous compaction, which the driver cancels before the next one.
    previous: Option<usize>,
}

impl ControlledDriver {
    /// A driver with a fresh host.
    pub fn new() -> Self {
        Self {
            host: Controlled::default(),
            flags: Vec::new(),
            previous: None,
        }
    }

    /// The component's exports served over the controlled host.
    fn exports(&self) -> Exports<Controlled> {
        Exports::new(self.host.clone())
    }

    /// The flag lent under a handle.
    fn flag(&self, signal: usize) -> Result<&Flag, String> {
        self.flags
            .get(signal)
            .ok_or_else(|| format!("no flag was lent as {signal}"))
    }

    /// The resources an event delivery lends: a context in `cwd` and the flag under `signal`.
    fn resources(
        &self,
        cwd: &str,
        signal: Option<usize>,
    ) -> Result<Capabilities<Controlled>, String> {
        let signal = signal
            .map(|signal| self.flag(signal).map(|flag| self.host.signal(flag)))
            .transpose()?;
        Ok(Capabilities {
            ctx: self.host.context(cwd),
            signal,
        })
    }

    /// Releases the registration next in line; whether there was one.
    fn release_next(&self) -> bool {
        let next = self.host.observe().next_to_release();
        next.map(|(_, key)| release(key)).is_some()
    }

    /// Runs `run` with the wait for idle it starts held pending until the driver saw it.
    async fn held<T>(&self, run: impl Future<Output = Result<T, String>>) -> Result<T, String> {
        let (hold, started, open) = gate();
        self.host.observe().hold_next_idle(hold);
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

    fn identity(&self, name: &str) -> Result<u32, String> {
        self.host.observed().callback(name)
    }

    async fn deliver(
        &mut self,
        handler: u32,
        event: &str,
        cwd: &str,
        signal: Option<usize>,
    ) -> Result<Delivery, String> {
        let resources = self.resources(cwd, signal)?;
        let outcome = self
            .exports()
            .invoke_event(handler, event.to_owned(), resources)
            .await?;
        Ok(fixtures::delivery(outcome))
    }

    async fn deliver_held(
        &mut self,
        handler: u32,
        event: &str,
        cwd: &str,
    ) -> Result<Delivery, String> {
        let resources = self.resources(cwd, None)?;
        let exports = self.exports();
        let outcome = self
            .held(exports.invoke_event(handler, event.to_owned(), resources))
            .await?;
        Ok(fixtures::delivery(outcome))
    }

    fn unregistered(&mut self) -> Result<u32, String> {
        Ok(UNKNOWN)
    }

    fn lend_signal(&mut self, aborted: bool) -> Result<usize, String> {
        let flag = Flag::default();
        if aborted {
            flag.abort();
        }
        self.flags.push(flag);
        Ok(self.flags.len() - 1)
    }

    fn abort_signal(&mut self, signal: usize) -> Result<(), String> {
        self.flag(signal).map(Flag::abort)
    }

    fn previous_signal(&mut self) -> &mut Option<usize> {
        &mut self.previous
    }

    fn prepare_key(
        &mut self,
        handler: u32,
        args: &str,
    ) -> impl Future<Output = Result<String, String>> {
        std::future::ready(Exports::<Controlled>::invoke_prepare(handler, args))
    }
    fn reborrow(&mut self, id: u32) -> Result<u32, String> {
        Ok(id)
    }

    async fn tool(&mut self, run: super::scenario::ToolRun<'_>) -> Result<String, String> {
        let handler = self.identity(&format!("tool {}", run.name))?;
        let invocation = crate::bindings::exports::maestro::extension::guest::ToolInvocation {
            tool_call_id: run.id.into(),
            params: run.params.into(),
        };
        let resources = crate::component_adapter::ToolCapabilities {
            ctx: self.host.context("/work"),
            signal: run
                .signal
                .map(|s| self.flag(s).map(|f| self.host.signal(f)))
                .transpose()?,
            update: run.update.then(|| self.host.update()),
        };
        let exports = self.exports();
        if run.held {
            let (hold, entered, open) = gate();
            self.host.observe().hold_next_idle(hold);
            let flag = run.signal.map(|s| self.flag(s).cloned()).transpose()?;
            let resume = cancel_after_entered(entered, flag, open);
            super::join::alongside(exports.invoke_tool(handler, invocation, resources), resume)
                .await
        } else {
            exports.invoke_tool(handler, invocation, resources).await
        }
    }

    fn tools(&self) -> Vec<String> {
        self.host.observed().tools.clone()
    }
    fn updates(&self) -> Vec<String> {
        self.host.observed().updates.clone()
    }

    async fn run_plain_command(&mut self, name: &str, args: &str) -> Result<(), String> {
        let handler = self.identity(&format!("command {name}"))?;
        let ctx = self.host.command_context("/work");
        self.exports()
            .invoke_command(handler, args.to_owned(), ctx)
            .await
    }

    async fn command(&mut self, args: &str) -> Result<(), String> {
        let handler = self.identity("command replace")?;
        let ctx = self.host.command_context("/work");
        let exports = self.exports();
        self.held(exports.invoke_command(handler, args.to_owned(), ctx))
            .await
    }

    fn release_all(&mut self) -> impl Future<Output = ()> {
        while self.release_next() {}
        std::future::ready(())
    }

    fn release(&mut self, name: &str) -> impl Future<Output = bool> {
        let key = self.host.observe().take_registration(name);
        std::future::ready(key.map(release).is_some())
    }

    fn reject_next_session(&mut self) {
        self.host.observe().reject_next_session();
    }

    async fn invoke_unknown_continuation(&mut self) -> String {
        let replaced = self.host.replaced_context("/replacement");
        let continuation = self.exports().invoke_with_session(UNKNOWN, replaced).await;
        continuation.err().unwrap_or_default()
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

/// Cancels only after the execution's entered barrier, then opens its release gate.
async fn cancel_after_entered(
    entered: tokio::sync::oneshot::Receiver<()>,
    flag: Option<Flag>,
    open: tokio::sync::oneshot::Sender<()>,
) {
    entered.await.expect("execution entered its wait");
    if let Some(flag) = flag {
        flag.abort();
    }
    open.send(()).expect("execution still pending");
}

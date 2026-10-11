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
    /// Independently returned component resources.
    /// Next invocation-local emitter identity.
    emitter_key: usize,
    components: Vec<Option<crate::Component>>,
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
            components: Vec::new(),
            emitter_key: 0,
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
    fn renderer_emissions(&self) -> Vec<(usize, String, serde_json::Value)> {
        self.host.observed().bus.renderer_emissions.clone()
    }
    fn renderer(
        &mut self,
        handler: u32,
        message: &str,
        expanded: bool,
        theme: &str,
    ) -> impl Future<Output = Result<Option<usize>, String>> {
        self.emitter_key += 1;
        let result = self
            .exports()
            .invoke_renderer(
                handler,
                crate::component_adapter::renderers::RenderRequest {
                    message,
                    expanded,
                    theme: self.host.theme(theme),
                    emitter: &super::bus_host::Emitter {
                        key: self.emitter_key,
                    },
                },
            )
            .map(|component| {
                component.map(|component| {
                    let id = self.components.len();
                    self.components.push(Some(component));
                    id
                })
            });
        std::future::ready(result)
    }
    fn component(
        &mut self,
        id: usize,
        call: super::renderer_host::Call<'_>,
    ) -> impl Future<Output = Result<super::renderer_host::Reply, String>> {
        self.emitter_key += 1;
        std::future::ready(
            self.components[id]
                .as_ref()
                .ok_or_else(|| "component absent".into())
                .and_then(|view| {
                    super::renderer_host::invoke(view, call, &self.host, self.emitter_key)
                }),
        )
    }
    fn drop_component(&mut self, id: usize) -> impl Future<Output = Result<(), String>> {
        drop(self.components[id].take());
        std::future::ready(Ok(()))
    }
    fn repeat_release(&mut self, handler: u32) -> impl Future<Output = Result<(), String>> {
        release(handler);
        std::future::ready(Ok(()))
    }

    fn notify_compaction(
        &mut self,
        handler: u32,
        payload: &str,
        complete: bool,
    ) -> impl Future<Output = Result<(), String>> {
        std::future::ready(if complete {
            self.exports().invoke_compaction_complete(
                handler,
                payload,
                &super::bus_host::Emitter { key: 0 },
            )
        } else {
            self.exports().invoke_compaction_error(
                handler,
                payload.to_owned(),
                &super::bus_host::Emitter { key: 0 },
            )
        })
    }
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
        std::future::ready(self.exports().invoke_prepare(
            handler,
            args,
            &super::bus_host::Emitter { key: 0 },
        ))
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
    fn reject_bus_emission(&mut self, error: Option<String>) {
        self.host.observe().bus.emission_error = error;
    }
    fn bus_emissions(&self) -> Vec<(String, serde_json::Value)> {
        self.host.observed().bus.emissions.clone()
    }
    fn updates(&self) -> Vec<String> {
        self.host.observed().updates.clone()
    }

    async fn completion(
        &mut self,
        handler: u32,
        prefix: &str,
    ) -> Result<Option<Vec<serde_json::Value>>, String> {
        self.exports().invoke_completion(handler, prefix.into()).await.map(|items| items.map(|items| items.into_iter().map(|item| serde_json::json!({"value":item.value, "label":item.label, "description":item.description})).collect()))
    }
    async fn completion_held(
        &mut self,
        handler: u32,
        prefix: &str,
    ) -> Result<Option<Vec<serde_json::Value>>, String> {
        let exports = self.exports();
        let run = exports.invoke_completion(handler, prefix.into());
        self.held(run).await.map(|items| items.map(|items| items.into_iter().map(|item| serde_json::json!({"value":item.value,"label":item.label,"description":item.description})).collect()))
    }
    async fn shortcut(&mut self, handler: u32, cwd: &str) -> Result<(), String> {
        self.exports()
            .invoke_shortcut(handler, self.host.context(cwd))
            .await
    }
    fn registrations(&self) -> super::scenario::Registrations {
        let observed = self.host.observed();
        (
            observed.registrations.commands.clone(),
            observed.registrations.shortcuts.clone(),
        )
    }
    async fn notify_setup(&mut self, handler: u32) -> Result<(), String> {
        self.exports()
            .invoke_setup(handler, self.host.writer("unused"))
            .await
    }
    async fn notify_continuation(&mut self, handler: u32) -> Result<(), String> {
        self.exports()
            .invoke_with_session(handler, self.host.replaced_context("identity-target"))
            .await
    }
    async fn drop_command(&mut self, name: &str, args: &str) -> Result<(), String> {
        let handler = self.identity(&format!("command {name}"))?;
        let exports = self.exports();
        let context = self.host.command_context("/work");
        let (finished, completion) = tokio::sync::oneshot::channel();
        self.host.observe().setup_finished = Some(finished);
        let (hold, entered, open) = gate();
        self.host.observe().hold_next_idle(hold);
        let main = async {
            let result = exports
                .invoke_command(handler, args.to_owned(), context)
                .await;
            completion.await.unwrap();
            result
        };
        let helper = async {
            entered.await.unwrap();
            let _ = open.send(());
        };
        super::join::alongside(main, helper).await
    }
    async fn host_drop(&mut self, args: &str) -> Result<(), String> {
        let handler = self.identity("command replacement")?;
        let exports = self.exports();
        let ctx = self.host.command_context("/work");
        let (done, finished) = tokio::sync::oneshot::channel();
        let (dropped, drop_seen) = tokio::sync::oneshot::channel();
        self.host.observe().replacement_finished = Some(done);
        self.host.observe().replacement_dropped = Some(dropped);
        let (operation, entered, open) = gate();
        let (notification, notified, acknowledge) = gate();
        self.host.observe().hold_next_idle(operation);
        self.host.observe().hold_next_idle(notification);
        let (complete, completed) = tokio::sync::oneshot::channel();
        let main = async {
            let result = exports.invoke_command(handler, args.to_owned(), ctx).await;
            completed.await.unwrap();
            result
        };
        let helper = async {
            entered.await.unwrap();
            notified.await.unwrap();
            acknowledge.send(()).unwrap();
            drop_seen.await.unwrap();
            let _ = open.send(());
            finished.await.unwrap();
            complete.send(()).unwrap();
        };
        super::join::alongside(main, helper).await
    }
    async fn setup_pair(&mut self) -> Result<(), String> {
        let handler = self.identity("command setup")?;
        let exports = self.exports();
        let first = self.host.command_context("/A");
        let second = self.host.command_context("/B");
        let start = self.host.observed().transcript.len();
        let (hold_a, entered_a, open_a) = gate();
        let (hold_b, entered_b, open_b) = gate();
        self.host.observe().hold_next_idle(hold_a);
        self.host.observe().hold_next_idle(hold_b);
        let (a, b) = super::join::reversed(
            exports.invoke_command(handler, super::setup_sessions::pair_input("A"), first),
            exports.invoke_command(handler, super::setup_sessions::pair_input("B"), second),
            [(entered_a, open_a), (entered_b, open_b)],
            |phase| {
                super::setup_sessions::pair_phase(&self.host.observed().transcript[start..], phase);
                std::future::ready(())
            },
        )
        .await;
        a?;
        b?;
        super::setup_sessions::pair_completed(&self.host.observed().transcript[start..]);
        Ok(())
    }
    async fn replacement_sends_pair(&mut self) -> Result<(), String> {
        let handler = self.identity("command replacement")?;
        let exports = self.exports();
        let first = self.host.command_context("/A");
        let second = self.host.command_context("/B");
        let start = self.host.observed().transcript.len();
        let (hold_a, entered_a, open_a) = gate();
        let (hold_b, entered_b, open_b) = gate();
        self.host.observe().hold_next_idle(hold_a);
        self.host.observe().hold_next_idle(hold_b);
        let (a, b) = super::join::reversed(
            exports.invoke_command(
                handler,
                super::replacement_commands::send_pair_input("A"),
                first,
            ),
            exports.invoke_command(
                handler,
                super::replacement_commands::send_pair_input("B"),
                second,
            ),
            [(entered_a, open_a), (entered_b, open_b)],
            |phase| {
                super::replacement_commands::send_pair_phase(
                    &self.host.observed().transcript[start..],
                    phase,
                );
                std::future::ready(())
            },
        )
        .await;
        a?;
        b?;
        super::replacement_commands::send_pair_completed(&self.host.observed().transcript[start..]);
        Ok(())
    }
    async fn replacement_triple(&mut self) -> Result<(), String> {
        let handler = self.identity("command replacement")?;
        let exports = self.exports();
        let start = self.transcript().len();
        let gates = std::array::from_fn(|_| gate());
        let barriers = gates.map(|(hold, entered, open)| {
            self.host.observe().hold_next_idle(hold);
            (entered, open)
        });
        let calls = ["newSession", "fork", "switchSession"].map(|group| {
            exports.invoke_command(
                handler,
                super::replacement_commands::triple_input(group),
                self.host.command_context(group),
            )
        });
        let outcomes = super::replacement_commands::triple(calls, barriers).await;
        for outcome in outcomes {
            outcome?;
        }
        super::replacement_commands::triple_completed(&self.transcript()[start..]);
        Ok(())
    }
    async fn pending_pair(&mut self, handler: u32, shortcut: bool) -> Result<(), String> {
        let exports = self.exports();
        let start = self.transcript().len();
        let (hold_first, entered_first, open_first) = gate();
        let (hold_second, entered_second, open_second) = gate();
        self.host.observe().hold_next_idle(hold_first);
        self.host.observe().hold_next_idle(hold_second);
        let call = async |origin: &str| {
            if shortcut {
                exports
                    .invoke_shortcut(handler, self.host.context(origin))
                    .await
            } else {
                let items = exports.invoke_completion(handler, origin.into()).await?;
                assert_eq!(items.unwrap()[0].value, origin);
                Ok(())
            }
        };
        let name = if shortcut {
            "shortcut pending"
        } else {
            "completion pending"
        };
        let observe = async |phase| {
            if matches!(phase, super::join::Phase::BothPending) {
                assert_eq!(self.host.observe().take_registration(name), Some(handler));
                release(handler);
            }
            self.host
                .observed()
                .assert_pending_phase(phase, start, shortcut);
        };
        let (first, second) = super::join::ordered(
            call("/first"),
            call("/second"),
            [(entered_first, open_first), (entered_second, open_second)],
            observe,
        )
        .await;
        first?;
        second?;
        Ok(())
    }
    fn callback_error(&self, handler: u32) -> String {
        format!("no callback registered for identity {handler}")
    }
    async fn command_held(&mut self, name: &str, args: &str) -> Result<(), String> {
        let handler = self.identity(&format!("command {name}"))?;
        let exports = self.exports();
        self.held(exports.invoke_command(handler, args.into(), self.host.command_context("/work")))
            .await
    }
    async fn shortcut_held(&mut self, handler: u32, cwd: &str) -> Result<(), String> {
        let exports = self.exports();
        self.held(exports.invoke_shortcut(handler, self.host.context(cwd)))
            .await
    }
    fn registration_error(&mut self, error: Option<String>) {
        self.host.observe().registrations.error = error;
    }
    fn flag_reply(&mut self, name: &str, value: Result<Option<serde_json::Value>, String>) {
        self.host
            .observe()
            .registrations
            .values
            .insert(name.into(), value);
    }
    fn flag_registrations(&self) -> Vec<(String, serde_json::Value)> {
        self.host.observed().registrations.flags.clone()
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

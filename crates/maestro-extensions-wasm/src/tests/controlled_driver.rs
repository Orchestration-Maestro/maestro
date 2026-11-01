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
    /// Independently returned shell resources.
    bash: Vec<Option<crate::BashOperations>>,
    /// Last returned shell resource slot.
    last_bash: Option<usize>,
    /// Next invocation-local emitter identity.
    emitter_key: usize,
    /// Independently returned component resources.
    components: Vec<Option<crate::Component>>,
    /// The flags lent so far, by handle.
    flags: Vec<Flag>,
    /// The flag of the previous compaction, which the driver cancels before the next one.
    previous: Option<usize>,
}

impl ControlledDriver {
    /// Delivers input to the factory widget the host shows under `key`.
    pub(super) fn widget_input(&self, key: &str, data: &str) -> Result<(), String> {
        let component = self
            .host
            .widget(key)
            .ok_or_else(|| format!("no widget {key}"))?;
        super::renderer_host::invoke(
            &component,
            super::renderer_host::Call::Input(data),
            &self.host,
            0,
        )
        .map(drop)
    }
    /// Starts a shipped factory with the existing controlled widget host installed.
    pub(super) async fn example<E: Extension>() -> Result<Self, String> {
        let driver = Self::new();
        driver.exports().start::<E>().await?;
        Ok(driver)
    }

    /// Publishes literal theme replies for a shipped factory's renderer.
    pub(super) fn publish_theme(&self, palette: super::theme_host::Palette) {
        self.host.observe().themes.publish("dark", palette);
    }

    /// A driver with a fresh host.
    pub fn new() -> Self {
        let host = Controlled::default();
        crate::component_adapter::install(std::rc::Rc::new(crate::component_adapter::Builder(
            host.clone(),
        )));
        Self {
            host,
            components: Vec::new(),
            bash: Vec::new(),
            last_bash: None,
            emitter_key: 0,
            flags: Vec::new(),
            previous: None,
        }
    }

    /// Releases the registration and cancels the signal after the first output and idle witness.
    async fn shell_pending(
        &self,
        state: (Option<u32>, usize, Option<usize>),
        gate: (
            tokio::sync::oneshot::Receiver<()>,
            tokio::sync::oneshot::Sender<()>,
        ),
    ) {
        let (Some(handler), output, signal) = state else {
            return;
        };
        let (pending, proceed) = gate;
        pending.await.unwrap();
        assert_eq!(self.shell_chunks(output), [vec![122]]);
        assert_eq!(
            self.host.observe().take_registration("event user_bash"),
            Some(handler)
        );
        release(handler);
        if let Some(flag) = signal {
            self.flag(flag).unwrap().abort();
        }
        proceed.send(()).unwrap();
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

impl ControlledDriver {
    /// Runs one tool renderer for its row, keeping a new row's state, and renders the result.
    fn rendered_tool(
        &self,
        run: &super::scenario::ToolRenderRun<'_>,
        width: u32,
    ) -> Result<Vec<String>, String> {
        use crate::bindings::maestro::extension::host;
        let handler = self.identity(&format!("render-{} {}", run.slot, run.tool))?;
        let wire = super::ui_host::render_info(run.info);
        let info = host::ToolRenderInfo {
            tool_call_id: wire.tool_call_id,
            args: wire.args,
            cwd: wire.cwd,
            phase: host::ToolRenderPhase {
                execution_started: wire.phase.execution_started,
                args_complete: wire.phase.args_complete,
                is_partial: wire.phase.is_partial,
            },
            expanded: wire.expanded,
            show_images: wire.show_images,
            is_error: wire.is_error,
        };
        let id = info.tool_call_id.clone();
        let slot = if run.slot == "call" {
            host::ToolRenderSlot::Call
        } else {
            host::ToolRenderSlot::Result
        };
        let render = host::ToolRender {
            slot,
            row: self.host.observed().tool_rows.get(&id).copied(),
            args: info.args.clone(),
            options: Some(host::ToolRenderResultOptions {
                expanded: info.expanded,
                is_partial: info.phase.is_partial,
            }),
            info,
            result: run.result.map(ToString::to_string),
        };
        let lent = (self.host.theme("dark"), self.host.invalidator(&id), None);
        let emitter = super::bus_host::Emitter { key: 0 };
        let (component, row) = self
            .exports()
            .invoke_tool_render(handler, &render, lent, &emitter)?;
        if let Some(row) = row {
            let row = crate::component_adapter::tool_renders::keep_row(&self.host, row);
            self.host.observe().tool_rows.insert(id, row);
        }
        let call = super::renderer_host::Call::Render(width);
        match super::renderer_host::invoke(&component?, call, &self.host, 0)? {
            super::renderer_host::Reply::Lines(lines) => Ok(lines),
            _ => Err("expected rendered lines".to_owned()),
        }
    }
}

impl Drop for ControlledDriver {
    /// The callback table is shared by the thread, so a driver leaves nothing behind.
    fn drop(&mut self) {
        while self.release_next() {}
    }
}

impl Driver for ControlledDriver {
    fn returned_bash(&self) -> Option<usize> {
        self.last_bash
    }
    fn shell_output(
        &mut self,
        error: Option<String>,
        cancel: Option<usize>,
    ) -> Result<usize, String> {
        let cancel = cancel
            .map(|id| self.flag(id).map(Flag::shared))
            .transpose()?;
        let mut observed = self.host.observe();
        let id = observed.shell.outputs.len();
        observed.shell.outputs.push(super::shell_host::Output {
            error,
            cancel,
            ..Default::default()
        });
        Ok(id)
    }
    fn shell_chunks(&self, id: usize) -> Vec<Vec<u8>> {
        self.host.observed().shell.outputs[id].chunks.clone()
    }
    fn shell_signal_drops(&self, id: usize) -> usize {
        self.host
            .observed()
            .shell
            .signal_drops
            .get(&self.flags[id].key())
            .copied()
            .unwrap_or(0)
    }
    fn shell_drops(&self, id: usize) -> usize {
        self.host.observed().shell.outputs[id].drops
    }
    async fn shell_exec(&mut self, run: super::shell_host::Run) -> Result<Option<f64>, String> {
        let operation = self.bash[run.operation]
            .as_ref()
            .ok_or("operation absent")?
            .clone();
        let request = fixtures::guest::BashRequest {
            command: run.command,
            cwd: run.cwd,
            timeout: run.timeout,
            env: run.env,
        };
        let signal = run
            .signal
            .map(|id| self.flag(id).map(|flag| self.host.signal(flag)))
            .transpose()?;
        let resources = crate::component_adapter::bash::BashCapabilities {
            output: self.host.bash_receiver(run.output),
            signal,
        };
        let exports = self.exports();
        let (pause, pending, proceed) = gate();
        if run.release_pending.is_some() {
            self.host.observe().hold_next_idle(pause);
        }
        let helper = self.shell_pending(
            (run.release_pending, run.output, run.signal),
            (pending, proceed),
        );
        super::join::alongside(exports.invoke_bash(&operation, request, resources), helper)
            .await
            .map(|r| r.exit_code)
    }
    fn drop_bash(&mut self, id: usize) -> impl Future<Output = Result<(), String>> {
        drop(self.bash[id].take());
        std::future::ready(Ok(()))
    }

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
        let mut outcome = self
            .exports()
            .invoke_event(handler, event.to_owned(), resources)
            .await?;
        self.last_bash = outcome.operations.take().map(|operation| {
            let id = self.bash.len();
            self.bash.push(Some(operation));
            id
        });
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
        let mut outcome = self
            .held(exports.invoke_event(handler, event.to_owned(), resources))
            .await?;
        self.last_bash = outcome.operations.take().map(|operation| {
            let id = self.bash.len();
            self.bash.push(Some(operation));
            id
        });
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

    fn render_tool(
        &mut self,
        run: super::scenario::ToolRenderRun<'_>,
        width: u32,
    ) -> impl Future<Output = Result<Vec<String>, String>> {
        std::future::ready(self.rendered_tool(&run, width))
    }
    fn footer(&mut self, width: u32) -> impl Future<Output = Result<Option<Vec<String>>, String>> {
        let rendered = self.host.footer().map(|component| {
            let call = super::renderer_host::Call::Render(width);
            match super::renderer_host::invoke(&component, call, &self.host, 0)? {
                super::renderer_host::Reply::Lines(lines) => Ok(lines),
                _ => Err("expected rendered lines".to_owned()),
            }
        });
        std::future::ready(rendered.transpose())
    }
    fn branch_change(&mut self) -> impl Future<Output = Result<Vec<serde_json::Value>, String>> {
        let listeners = super::ui_host::listeners_on(&self.host.observed(), "branch");
        let answers = listeners
            .into_iter()
            .map(|id| {
                super::ui_host::unit_answer(
                    crate::component_adapter::factories::invoke_branch_change(id),
                )
            })
            .collect();
        std::future::ready(Ok(answers))
    }
    fn header(
        &mut self,
        width: u32,
    ) -> impl Future<Output = Result<(Vec<String>, Option<Vec<String>>), String>> {
        let children = self.host.observed().ui.header.children.clone();
        let rendered = self.host.header().map(|component| {
            let call = super::renderer_host::Call::Render(width);
            match super::renderer_host::invoke(&component, call, &self.host, 0)? {
                super::renderer_host::Reply::Lines(lines) => Ok(lines),
                _ => Err("expected rendered lines".to_owned()),
            }
        });
        std::future::ready(rendered.transpose().map(|rendered| (children, rendered)))
    }
    fn widget(
        &mut self,
        key: &str,
        width: u32,
    ) -> impl Future<Output = Result<Vec<String>, String>> {
        let rendered = self
            .host
            .widget(key)
            .ok_or_else(|| format!("no widget {key}"))
            .and_then(|component| {
                let call = super::renderer_host::Call::Render(width);
                super::renderer_host::invoke(&component, call, &self.host, 0)
            });
        std::future::ready(rendered.and_then(|reply| match reply {
            super::renderer_host::Reply::Lines(lines) => Ok(lines),
            _ => Err("expected rendered lines".to_owned()),
        }))
    }
    fn editor_request(
        &mut self,
        request: serde_json::Value,
    ) -> impl Future<Output = Result<serde_json::Value, String>> {
        std::future::ready(self.host.authored_editor(request))
    }
    async fn provider_request(
        &mut self,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.host.provider_request(request, &self.flags).await
    }
    fn editor_component(
        &mut self,
        call: super::renderer_host::Call<'_>,
    ) -> impl Future<Output = Result<super::renderer_host::Reply, String>> {
        std::future::ready(self.host.editor_component(call))
    }
    fn widget_capabilities(
        &mut self,
        key: &str,
        expanded: bool,
    ) -> impl Future<Output = Result<(Option<f64>, bool), String>> {
        let answer = (|| {
            let built = self.host.widget_built(key).ok_or("no factory component")?;
            let present = built.set_expanded.is_some();
            if let Some(handler) = built.set_expanded {
                let emitter = super::bus_host::Emitter { key: 0 };
                crate::component_adapter::renderers::with_emitter(
                    &self.host,
                    &emitter,
                    |emitter| handler(expanded, emitter),
                )?;
            }
            Ok((built.width, present))
        })();
        std::future::ready(answer)
    }
    fn terminal_input(
        &mut self,
        data: &str,
    ) -> impl Future<Output = Result<Vec<serde_json::Value>, String>> {
        let listeners = super::ui_host::terminal_listeners(&self.host.observed());
        let exports = self.exports();
        let emitter = super::bus_host::Emitter { key: 0 };
        let answers = listeners
            .into_iter()
            .map(|id| {
                let answer = exports.invoke_terminal_input(id, data, &emitter);
                super::ui_host::terminal_answer(
                    answer.map(|decision| decision.map(|it| (it.consume, it.data))),
                )
            })
            .collect();
        std::future::ready(Ok(answers))
    }
    fn prepare_key(
        &mut self,
        handler: u32,
        args: &str,
    ) -> impl Future<Output = Result<String, String>> {
        std::future::ready(
            self.exports()
                .invoke_prepare(handler, args, &super::bus_host::Emitter { key: 0 })
                .and_then(|result| result.prepared),
        )
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

/// The native adapter dispatches only the selected callback against its actual borrowed value.
#[test]
fn controlled_schema_resource_invokes_the_supplied_refinement() {
    use crate::bindings::maestro::extension::host::SchemaOperation;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    let mut driver = ControlledDriver::new();
    super::block_on(driver.start()).unwrap();
    let config = json!({"metadata":{"name":"original","label":"Original","description":"","parameters":{"type":"object"}},
        "schemaProgram":"full","prepare":false,"result":{"content":[]}});
    super::block_on(driver.run_plain_command("capture", &format!("tools {config}"))).unwrap();
    let handler = driver
        .host
        .observe()
        .schemas
        .iter()
        .find(|(name, _, _)| name == "original")
        .unwrap()
        .2;
    let shape = Exports::<Controlled>::describe_schema(handler).unwrap();
    assert!(shape.base);
    assert_eq!(shape.refinements, 2);
    let value = Arc::new(Mutex::new(json!({"original":1})));
    let result = driver
        .exports()
        .invoke_schema(
            handler,
            SchemaOperation::Refine(1),
            &super::schema_host::JsonNode(Arc::clone(&value)),
            &super::bus_host::Emitter { key: 37 },
        )
        .unwrap();
    assert_eq!(result, "false");
    assert_eq!(*value.lock().unwrap(), json!({"original":1,"refine1":true}));
    release(handler);
    super::block_on(driver.release_all());
}

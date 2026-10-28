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
    /// Independently returned shell resources.
    bash: Vec<Option<Resource<super::host::Identity>>>,
    /// Last returned shell resource slot.
    last_bash: Option<usize>,
    /// Next invocation-local emitter identity.
    emitter_key: usize,
    /// Independently returned guest components, identified by their shared identity.
    components: Vec<Option<Resource<super::host::Identity>>>,
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
    /// Releases an authored object the extension returned, then drops its host identity.
    async fn drop_object(
        &mut self,
        object: Option<Resource<super::host::Identity>>,
    ) -> Result<(), String> {
        let Some(object) = object else {
            return Ok(());
        };
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        harness
            .release(object.rep())
            .await
            .map_err(|e| e.to_string())?;
        crate::bindings::host_side::maestro::extension::host::HostCallback::drop(
            harness.store.data_mut(),
            object,
        )
        .map_err(|e| e.to_string())
    }

    /// A driver for the component at `path`.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            harness: None,
            components: Vec::new(),
            bash: Vec::new(),
            last_bash: None,
            emitter_key: 0,
            flags: Vec::new(),
            previous: None,
        }
    }

    /// Publishes literal theme replies for the running component's renderer.
    pub(super) fn publish_theme(
        &mut self,
        palette: super::theme_host::Palette,
    ) -> Result<(), String> {
        started(&mut self.harness)
            .map_err(|error| error.to_string())?
            .observed()
            .themes
            .publish("dark", palette);
        Ok(())
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

    /// Renders the host-held component with the table handle `rep`.
    async fn render_slot(&mut self, rep: u32, width: u32) -> Result<Vec<String>, String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let emitter = harness
            .store
            .data_mut()
            .table
            .push(super::bus_host::Emitter { key: 0 })
            .map_err(|e| e.to_string())?;
        let call = super::renderer_host::Call::Render(width);
        let reply = component_call(harness, Resource::new_borrow(rep), call, &emitter).await?;
        harness
            .store
            .data_mut()
            .table
            .delete(emitter)
            .map_err(|e| e.to_string())?;
        match reply? {
            super::renderer_host::Reply::Lines(lines) => Ok(lines),
            _ => Err("expected rendered lines".to_owned()),
        }
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
    fn returned_bash(&self) -> Option<usize> {
        self.last_bash
    }
    fn shell_output(
        &mut self,
        error: Option<String>,
        cancel: Option<usize>,
    ) -> Result<usize, String> {
        let key = cancel
            .map(|id| self.key(id))
            .transpose()
            .map_err(|e| e.to_string())?;
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let state = harness.store.data_mut();
        let cancel = key
            .map(|key| {
                state
                    .table
                    .get(&Resource::<super::host::Flag>::new_borrow(key))
                    .map(|flag| flag.0.clone())
            })
            .transpose()
            .map_err(|e| e.to_string())?;
        let id = state.observed.shell.outputs.len();
        state
            .observed
            .shell
            .outputs
            .push(super::shell_host::Output {
                error,
                cancel,
                ..Default::default()
            });
        Ok(id)
    }
    fn shell_chunks(&self, id: usize) -> Vec<Vec<u8>> {
        self.observed(|o| o.shell.outputs[id].chunks.clone())
    }
    fn shell_signal_drops(&self, id: usize) -> usize {
        self.observed(|o| {
            o.shell
                .signal_drops
                .get(&(self.flags[id] as usize))
                .copied()
                .unwrap_or(0)
        })
    }
    fn shell_drops(&self, id: usize) -> usize {
        self.observed(|o| o.shell.outputs[id].drops)
    }
    async fn shell_exec(&mut self, run: super::shell_host::Run) -> Result<Option<f64>, String> {
        let operation = {
            let stored = self.bash[run.operation]
                .as_ref()
                .ok_or("operation absent")?;
            Resource::<super::host::Identity>::new_borrow(stored.rep())
        };
        let key = run
            .signal
            .map(|id| self.key(id))
            .transpose()
            .map_err(|e| e.to_string())?;
        let request = fixtures::guest::BashRequest {
            command: run.command,
            cwd: run.cwd,
            timeout: run.timeout,
            env: run.env,
        };
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let signal = key.map(Resource::new_own);
        let output = harness
            .store
            .data_mut()
            .push(super::shell_host::Receiver(run.output))
            .map_err(|e| e.to_string())?;
        let resources = fixtures::guest::BashCapabilities { output, signal };
        let (pause, pending, proceed) = gate();
        if run.release_pending.is_some() {
            harness.observed().hold_next_idle(pause);
        }
        let exports = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                let helper = shell_pending(
                    accessor,
                    &exports,
                    (run.release_pending, run.output, key),
                    (pending, proceed),
                );
                super::join::alongside(
                    exports.call_bash_exec(accessor, operation, request, resources),
                    helper,
                )
                .await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }
    async fn drop_bash(&mut self, id: usize) -> Result<(), String> {
        let operation = self.bash[id].take();
        self.drop_object(operation).await
    }

    fn renderer_emissions(&self) -> Vec<(usize, String, serde_json::Value)> {
        self.observed(|o| o.bus.renderer_emissions.clone())
    }
    async fn renderer(
        &mut self,
        handler: u32,
        message: &str,
        expanded: bool,
        theme: &str,
    ) -> Result<Option<usize>, String> {
        self.emitter_key += 1;
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let theme = harness
            .store
            .data_mut()
            .push(super::renderer_host::Theme(theme.into()))
            .map_err(|e| e.to_string())?;
        let emitter = harness
            .store
            .data_mut()
            .table
            .push(super::bus_host::Emitter {
                key: self.emitter_key,
            })
            .map_err(|e| e.to_string())?;
        let result = harness
            .exports
            .func_invoke_renderer()
            .call_async(
                &mut harness.store,
                (
                    borrow(handler),
                    fixtures::guest::RenderRequest {
                        message: message.into(),
                        expanded,
                        theme,
                    },
                    Resource::new_borrow(emitter.rep()),
                ),
            )
            .await
            .map_err(|e| e.to_string())?
            .0;
        harness
            .store
            .data_mut()
            .table
            .delete(emitter)
            .map_err(|e| e.to_string())?;
        Ok(result?.map(|component| {
            let id = self.components.len();
            self.components.push(Some(component));
            id
        }))
    }
    async fn component(
        &mut self,
        id: usize,
        call: super::renderer_host::Call<'_>,
    ) -> Result<super::renderer_host::Reply, String> {
        self.emitter_key += 1;
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let component = {
            let stored = self.components[id].as_ref().ok_or("component absent")?;
            Resource::<super::host::Identity>::new_borrow(stored.rep())
        };
        let emitter = harness
            .store
            .data_mut()
            .table
            .push(super::bus_host::Emitter {
                key: self.emitter_key,
            })
            .map_err(|e| e.to_string())?;
        let result = component_call(harness, component, call, &emitter).await?;
        harness
            .store
            .data_mut()
            .table
            .delete(emitter)
            .map_err(|e| e.to_string())?;
        result
    }
    async fn drop_component(&mut self, id: usize) -> Result<(), String> {
        let component = self.components[id].take();
        self.drop_object(component).await
    }

    async fn repeat_release(&mut self, handler: u32) -> Result<(), String> {
        started(&mut self.harness)
            .map_err(|e| e.to_string())?
            .release(handler)
            .await
            .map_err(|e| e.to_string())
    }

    async fn notify_compaction(
        &mut self,
        handler: u32,
        payload: &str,
        complete: bool,
    ) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        super::host::notify_compaction(harness, handler, payload, complete).await
    }
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
        let mut outcome = self
            .call(handler, event, resources, false)
            .await
            .map_err(|error| error.to_string())??;
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
        let resources = self
            .resources(cwd, None)
            .map_err(|error| error.to_string())?;
        let mut outcome = self
            .call(handler, event, resources, true)
            .await
            .map_err(|error| error.to_string())??;
        self.last_bash = outcome.operations.take().map(|operation| {
            let id = self.bash.len();
            self.bash.push(Some(operation));
            id
        });
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

    async fn render_tool(
        &mut self,
        run: super::scenario::ToolRenderRun<'_>,
        width: u32,
    ) -> Result<Vec<String>, String> {
        use crate::bindings::host_side::maestro::extension::host;
        let handler = self.identity(&format!("render-{} {}", run.slot, run.tool))?;
        let info = super::ui_host::render_info(run.info);
        let id = info.tool_call_id.clone();
        let row = self.observed(|observed| observed.tool_rows.get(&id).copied());
        let slot = if run.slot == "call" {
            host::ToolRenderSlot::Call
        } else {
            host::ToolRenderSlot::Result
        };
        let render = host::ToolRender {
            slot,
            row,
            args: info.args.clone(),
            options: Some(host::ToolRenderResultOptions {
                expanded: info.expanded,
                is_partial: info.phase.is_partial,
            }),
            info,
            result: run.result.map(ToString::to_string),
        };
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let state = harness.store.data_mut();
        let theme = state.push(super::renderer_host::Theme("dark".into()));
        let invalidator = state.push(super::ui_host::Invalidator(id.clone()));
        let emitter = state.table.push(super::bus_host::Emitter { key: 0 });
        let (theme, invalidator, emitter) = (
            theme.map_err(|e| e.to_string())?,
            invalidator.map_err(|e| e.to_string())?,
            emitter.map_err(|e| e.to_string())?,
        );
        let operands = (
            borrow(handler),
            render,
            theme,
            invalidator,
            None,
            Resource::new_borrow(emitter.rep()),
        );
        let rendered = harness
            .exports
            .func_invoke_tool_render()
            .call_async(&mut harness.store, operands)
            .await
            .map_err(|e| e.to_string())?
            .0?;
        let state = harness.store.data_mut();
        state.table.delete(emitter).map_err(|e| e.to_string())?;
        if let Some(row) = rendered.row {
            let row = state.table.get(&row).map_err(|e| e.to_string())?.0;
            state.observed.tool_rows.insert(id, row);
        }
        let component = rendered.component?.rep();
        state
            .observed
            .register(format!("tool-component {component}"), component);
        self.render_slot(component, width).await
    }

    async fn footer(&mut self, width: u32) -> Result<Option<Vec<String>>, String> {
        let custom = self.observed(|observed| {
            observed
                .ui
                .footer
                .custom
                .as_ref()
                .map(|slot| slot.component)
        });
        match custom {
            Some(rep) => self.render_slot(rep, width).await.map(Some),
            None => Ok(None),
        }
    }

    async fn branch_change(&mut self) -> Result<Vec<serde_json::Value>, String> {
        let listeners = self.observed(|observed| super::ui_host::listeners_on(observed, "branch"));
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let mut answers = Vec::new();
        for id in listeners {
            let emitter = harness
                .store
                .data_mut()
                .table
                .push(super::bus_host::Emitter { key: 0 })
                .map_err(|e| e.to_string())?;
            let operands = (borrow(id), Resource::new_borrow(emitter.rep()));
            let answer = harness
                .exports
                .func_invoke_branch_change()
                .call_async(&mut harness.store, operands)
                .await
                .map_err(|e| e.to_string())?
                .0;
            harness
                .store
                .data_mut()
                .table
                .delete(emitter)
                .map_err(|e| e.to_string())?;
            answers.push(super::ui_host::unit_answer(answer));
        }
        Ok(answers)
    }

    async fn header(&mut self, width: u32) -> Result<(Vec<String>, Option<Vec<String>>), String> {
        let (children, custom) = self.observed(|observed| {
            let header = &observed.ui.header;
            (
                header.children.clone(),
                header.custom.as_ref().map(|slot| slot.component),
            )
        });
        let Some(rep) = custom else {
            return Ok((children, None));
        };
        let rendered = self.render_slot(rep, width).await?;
        Ok((children, Some(rendered)))
    }

    async fn widget(&mut self, key: &str, width: u32) -> Result<Vec<String>, String> {
        let slot = self.observed(|observed| {
            observed
                .ui
                .widgets
                .iter()
                .find(|slot| slot.key == key)
                .map(|slot| slot.component)
        });
        let rep = slot.ok_or_else(|| format!("no widget {key}"))?;
        self.render_slot(rep, width).await
    }
    async fn editor_request(
        &mut self,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let op = super::editor_host::op(request)?;
        let editor = self.observed(|observed| observed.ui.editor.as_ref().map(|slot| slot.editor));
        let editor = editor.ok_or("no authored editor")?;
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        super::editor_driver::request(harness, editor, op)
            .await
            .map_err(|e| e.to_string())?
    }
    async fn provider_request(
        &mut self,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let op = super::provider_host::op(request)?;
        let top = self.observed(|observed| super::provider_host::top(observed).ok());
        let top = top.ok_or("no authored provider")?;
        let signal = match &op {
            super::provider_host::ProviderOp::Suggestions { signal, .. } => {
                Some(self.key(*signal).map_err(|e| e.to_string())?)
            }
            _ => None,
        };
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        super::editor_driver::provider(harness, top, op, signal)
            .await
            .map_err(|e| e.to_string())?
    }
    async fn editor_component(
        &mut self,
        call: super::renderer_host::Call<'_>,
    ) -> Result<super::renderer_host::Reply, String> {
        let component =
            self.observed(|observed| observed.ui.editor.as_ref().map(|slot| slot.component));
        let rep = component.ok_or("no authored editor")?;
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let emitter = harness
            .store
            .data_mut()
            .table
            .push(super::bus_host::Emitter { key: 0 })
            .map_err(|e| e.to_string())?;
        let reply = component_call(harness, Resource::new_borrow(rep), call, &emitter).await?;
        harness
            .store
            .data_mut()
            .table
            .delete(emitter)
            .map_err(|e| e.to_string())?;
        reply
    }
    async fn widget_capabilities(
        &mut self,
        key: &str,
        expanded: bool,
    ) -> Result<(Option<f64>, bool), String> {
        let (width, handler) = self.observed(|observed| {
            let slot = observed
                .ui
                .widgets
                .iter()
                .find(|slot| slot.key == key)
                .unwrap();
            (slot.width, slot.set_expanded)
        });
        let Some(handler) = handler else {
            return Ok((width, false));
        };
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let emitter = harness
            .store
            .data_mut()
            .table
            .push(super::bus_host::Emitter { key: 0 })
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let answer = harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_set_expanded(
                        accessor,
                        borrow(handler),
                        expanded,
                        Resource::new_borrow(emitter.rep()),
                    )
                    .await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        harness
            .store
            .data_mut()
            .table
            .delete(emitter)
            .map_err(|e| e.to_string())?;
        answer?;
        Ok((width, true))
    }
    async fn terminal_input(&mut self, data: &str) -> Result<Vec<serde_json::Value>, String> {
        let listeners = self.observed(super::ui_host::terminal_listeners);
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let mut answers = Vec::new();
        for id in listeners {
            let emitter = harness
                .store
                .data_mut()
                .table
                .push(super::bus_host::Emitter { key: 0 })
                .map_err(|e| e.to_string())?;
            let operands = (
                borrow(id),
                data.to_owned(),
                Resource::new_borrow(emitter.rep()),
            );
            let answer = harness
                .exports
                .func_invoke_terminal_input()
                .call_async(&mut harness.store, operands)
                .await
                .map_err(|e| e.to_string())?
                .0;
            harness
                .store
                .data_mut()
                .table
                .delete(emitter)
                .map_err(|e| e.to_string())?;
            answers.push(super::ui_host::terminal_answer(
                answer.map(|decision| decision.map(|it| (it.consume, it.data))),
            ));
        }
        Ok(answers)
    }
    async fn prepare_key(&mut self, handler: u32, args: &str) -> Result<String, String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let emitter = harness
            .store
            .data_mut()
            .table
            .push(super::bus_host::Emitter { key: 0 })
            .map_err(|e| e.to_string())?;
        let result = harness
            .exports
            .func_invoke_prepare()
            .call_async(
                &mut harness.store,
                (
                    borrow(handler),
                    args.to_owned(),
                    wasmtime::component::Resource::new_borrow(emitter.rep()),
                ),
            )
            .await
            .map_err(|e| e.to_string())?
            .0;
        harness
            .store
            .data_mut()
            .table
            .delete(emitter)
            .map_err(|e| e.to_string())?;
        result.and_then(|result| result.prepared)
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
    fn reject_bus_emission(&mut self, error: Option<String>) {
        self.harness
            .as_mut()
            .expect("started component")
            .store
            .data_mut()
            .observed
            .bus
            .emission_error = error;
    }
    fn bus_emissions(&self) -> Vec<(String, serde_json::Value)> {
        self.harness
            .as_ref()
            .expect("started component")
            .store
            .data()
            .observed
            .bus
            .emissions
            .clone()
    }
    fn updates(&self) -> Vec<String> {
        self.observed(|o| o.updates.clone())
    }

    async fn completion(
        &mut self,
        handler: u32,
        prefix: &str,
    ) -> Result<Option<Vec<serde_json::Value>>, String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let result = harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_completion(accessor, borrow(handler), prefix.to_owned())
                    .await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        result.map(|items| items.map(|items| items.into_iter().map(|item| serde_json::json!({"value":item.value, "label":item.label, "description":item.description})).collect()))
    }
    async fn completion_held(
        &mut self,
        handler: u32,
        prefix: &str,
    ) -> Result<Option<Vec<serde_json::Value>>, String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let (pause, pending, proceed) = gate();
        harness.observed().hold_next_idle(pause);
        let exports = harness.exports.clone();
        let result = harness
            .store
            .run_concurrent(async |accessor| {
                let call =
                    exports.call_invoke_completion(accessor, borrow(handler), prefix.to_owned());
                super::join::alongside(call, release_held(accessor, Some((pending, proceed)))).await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        result.map(|items| items.map(|items| items.into_iter().map(|item| serde_json::json!({"value":item.value,"label":item.label,"description":item.description})).collect()))
    }
    async fn shortcut(&mut self, handler: u32, cwd: &str) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let ctx = harness.ordinary(cwd).map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_shortcut(accessor, borrow(handler), ctx)
                    .await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }
    fn registrations(&self) -> super::scenario::Registrations {
        self.observed(|o| {
            (
                o.registrations.commands.clone(),
                o.registrations.shortcuts.clone(),
            )
        })
    }
    async fn notify_setup(&mut self, handler: u32) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let writer = harness
            .store
            .data_mut()
            .push(super::setup_host::reader("unused"))
            .map_err(|e| e.to_string())?;
        harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_setup(accessor, borrow(handler), writer)
                    .await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }
    async fn notify_continuation(&mut self, handler: u32) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let target = harness
            .replaced("identity-target")
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_with_session(accessor, borrow(handler), target)
                    .await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }
    async fn drop_command(&mut self, name: &str, args: &str) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let handler = harness
            .callback(&format!("command {name}"))
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let context = harness
            .command_context("/work")
            .map_err(|e| e.to_string())?;
        let (finished, completion) = tokio::sync::oneshot::channel();
        harness.observed().setup_finished = Some(finished);
        let (hold, entered, open) = gate();
        harness.observed().hold_next_idle(hold);
        harness
            .store
            .run_concurrent(async |accessor| {
                let main = async {
                    let result = exports
                        .call_invoke_command(accessor, borrow(handler), args.to_owned(), context)
                        .await;
                    completion.await.unwrap();
                    result
                };
                let helper = async {
                    entered.await.unwrap();
                    let _ = open.send(());
                };
                super::join::alongside(main, helper).await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }
    async fn host_drop(&mut self, args: &str) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let handler = harness
            .callback("command replacement")
            .map_err(|e| e.to_string())?;
        let ctx = harness
            .command_context("/work")
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let (done, finished) = tokio::sync::oneshot::channel();
        let (dropped, drop_seen) = tokio::sync::oneshot::channel();
        harness.observed().replacement_finished = Some(done);
        harness.observed().replacement_dropped = Some(dropped);
        let (operation, entered, open) = gate();
        let (notification, notified, acknowledge) = gate();
        harness.observed().hold_next_idle(operation);
        harness.observed().hold_next_idle(notification);
        harness
            .store
            .run_concurrent(async |accessor| {
                let (complete, completed) = tokio::sync::oneshot::channel();
                let main = async {
                    let result = exports
                        .call_invoke_command(accessor, borrow(handler), args.to_owned(), ctx)
                        .await;
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
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }
    async fn setup_pair(&mut self) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let handler = harness
            .callback("command setup")
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let first = harness.command_context("/A").map_err(|e| e.to_string())?;
        let second = harness.command_context("/B").map_err(|e| e.to_string())?;
        let start = harness.store.data().observed.transcript.len();
        let (hold_a, entered_a, open_a) = gate();
        let (hold_b, entered_b, open_b) = gate();
        harness.observed().hold_next_idle(hold_a);
        harness.observed().hold_next_idle(hold_b);
        harness
            .store
            .run_concurrent(async |accessor| {
                let observe = async |phase| {
                    setup_phase(accessor, start, phase);
                };
                let (a, b) = super::join::reversed(
                    exports.call_invoke_command(
                        accessor,
                        borrow(handler),
                        super::setup_sessions::pair_input("A"),
                        first,
                    ),
                    exports.call_invoke_command(
                        accessor,
                        borrow(handler),
                        super::setup_sessions::pair_input("B"),
                        second,
                    ),
                    [(entered_a, open_a), (entered_b, open_b)],
                    observe,
                )
                .await;
                a.map_err(|e| e.to_string())??;
                b.map_err(|e| e.to_string())??;
                accessor.with(|mut a| {
                    super::setup_sessions::pair_completed(&a.get().observed.transcript[start..]);
                });
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?
    }
    async fn replacement_sends_pair(&mut self) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let handler = harness
            .callback("command replacement")
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let first = harness.command_context("/A").map_err(|e| e.to_string())?;
        let second = harness.command_context("/B").map_err(|e| e.to_string())?;
        let start = harness.store.data().observed.transcript.len();
        let (hold_a, entered_a, open_a) = gate();
        let (hold_b, entered_b, open_b) = gate();
        harness.observed().hold_next_idle(hold_a);
        harness.observed().hold_next_idle(hold_b);
        harness
            .store
            .run_concurrent(async |accessor| {
                let observe = async |phase| {
                    send_pair_phase(accessor, start, phase);
                };
                let (a, b) = super::join::reversed(
                    exports.call_invoke_command(
                        accessor,
                        borrow(handler),
                        super::replacement_commands::send_pair_input("A"),
                        first,
                    ),
                    exports.call_invoke_command(
                        accessor,
                        borrow(handler),
                        super::replacement_commands::send_pair_input("B"),
                        second,
                    ),
                    [(entered_a, open_a), (entered_b, open_b)],
                    observe,
                )
                .await;
                a.map_err(|e| e.to_string())??;
                b.map_err(|e| e.to_string())??;
                accessor.with(|mut a| {
                    super::replacement_commands::send_pair_completed(
                        &a.get().observed.transcript[start..],
                    );
                });
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?
    }
    async fn replacement_triple(&mut self) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let handler = harness
            .callback("command replacement")
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        let start = harness.store.data().observed.transcript.len();
        let gates = std::array::from_fn(|_| gate());
        let barriers = gates.map(|(hold, entered, open)| {
            harness.observed().hold_next_idle(hold);
            (entered, open)
        });
        let contexts = ["newSession", "fork", "switchSession"]
            .map(|group| harness.command_context(group).unwrap());
        harness
            .store
            .run_concurrent(async |accessor| {
                let groups = ["newSession", "fork", "switchSession"];
                let calls = contexts
                    .into_iter()
                    .zip(groups)
                    .map(|(ctx, group)| {
                        exports.call_invoke_command(
                            accessor,
                            borrow(handler),
                            super::replacement_commands::triple_input(group),
                            ctx,
                        )
                    })
                    .collect::<Vec<_>>();
                let outcomes = super::replacement_commands::triple(
                    calls
                        .try_into()
                        .unwrap_or_else(|_| panic!("three operations")),
                    barriers,
                )
                .await;
                for outcome in outcomes {
                    outcome.map_err(|e| e.to_string())??;
                }
                accessor.with(|mut a| {
                    super::replacement_commands::triple_completed(
                        &a.get().observed.transcript[start..],
                    );
                });
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?
    }
    async fn pending_pair(&mut self, handler: u32, shortcut: bool) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let start = harness.store.data().observed.transcript.len();
        let (hold_first, entered_first, open_first) = gate();
        let (hold_second, entered_second, open_second) = gate();
        harness.observed().hold_next_idle(hold_first);
        harness.observed().hold_next_idle(hold_second);
        let first_ctx = shortcut
            .then(|| harness.ordinary("/first"))
            .transpose()
            .map_err(|e| e.to_string())?;
        let second_ctx = shortcut
            .then(|| harness.ordinary("/second"))
            .transpose()
            .map_err(|e| e.to_string())?;
        let exports = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                let observe = async |phase| {
                    pending_observation(accessor, &exports, (handler, start, shortcut), phase)
                        .await;
                };
                let (first, second) = super::join::ordered(
                    pending_call(accessor, &exports, handler, "/first", first_ctx),
                    pending_call(accessor, &exports, handler, "/second", second_ctx),
                    [(entered_first, open_first), (entered_second, open_second)],
                    observe,
                )
                .await;
                first?;
                second?;
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?
    }
    fn callback_error(&self, handler: u32) -> String {
        self.harness.as_ref().unwrap().callback_error(handler)
    }
    async fn command_held(&mut self, name: &str, args: &str) -> Result<(), String> {
        self.run_command(name, args, true)
            .await
            .map_err(|e| format!("{e:#}"))
    }
    async fn shortcut_held(&mut self, handler: u32, cwd: &str) -> Result<(), String> {
        let harness = started(&mut self.harness).map_err(|e| e.to_string())?;
        let ctx = harness.ordinary(cwd).map_err(|e| e.to_string())?;
        let (pause, pending, proceed) = gate();
        harness.observed().hold_next_idle(pause);
        let exports = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                let call = exports.call_invoke_shortcut(accessor, borrow(handler), ctx);
                super::join::alongside(call, release_held(accessor, Some((pending, proceed)))).await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    }
    fn registration_error(&mut self, error: Option<String>) {
        started(&mut self.harness)
            .unwrap()
            .observed()
            .registrations
            .error = error;
    }
    fn flag_reply(&mut self, name: &str, value: Result<Option<serde_json::Value>, String>) {
        started(&mut self.harness)
            .unwrap()
            .observed()
            .registrations
            .values
            .insert(name.into(), value);
    }
    fn flag_registrations(&self) -> Vec<(String, serde_json::Value)> {
        self.observed(|o| o.registrations.flags.clone())
    }
    async fn run_plain_command(&mut self, name: &str, args: &str) -> Result<(), String> {
        self.run_command(name, args, false)
            .await
            .map_err(|error| format!("{error:#}"))
    }

    async fn command(&mut self, args: &str) -> Result<(), String> {
        self.run_command("replace", args, true)
            .await
            .map_err(|error| error.to_string())
    }

    async fn release_all(&mut self) {
        for id in 0..self.components.len() {
            let _ = self.drop_component(id).await;
        }
        for id in 0..self.bash.len() {
            let _ = self.drop_bash(id).await;
        }
        let Ok(harness) = started(&mut self.harness) else {
            return;
        };
        for (name, outcome) in harness.release_all().await {
            if let Err(error) = outcome {
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

/// Calls an admitted completion or shortcut with an independently bound input.
async fn pending_call(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    exports: &crate::bindings::host_side::exports::maestro::extension::guest::Guest,
    handler: u32,
    origin: &str,
    ctx: Option<Resource<super::host::Ordinary>>,
) -> Result<(), String> {
    if let Some(ctx) = ctx {
        exports
            .call_invoke_shortcut(accessor, borrow(handler), ctx)
            .await
            .map_err(|e| e.to_string())?
    } else {
        let items = exports
            .call_invoke_completion(accessor, borrow(handler), origin.into())
            .await
            .map_err(|e| e.to_string())??;
        assert_eq!(items.unwrap()[0].value, origin);
        Ok(())
    }
}

/// Releases the registration at the pending barrier, then checks the closed observation set.
async fn pending_observation(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    exports: &crate::bindings::host_side::exports::maestro::extension::guest::Guest,
    state: (u32, usize, bool),
    phase: super::join::Phase,
) {
    let (handler, start, shortcut) = state;
    if matches!(phase, super::join::Phase::BothPending) {
        let name = if shortcut {
            "shortcut pending"
        } else {
            "completion pending"
        };
        accessor.with(|mut a| {
            assert_eq!(a.get().observed.take_registration(name), Some(handler));
        });
        exports
            .func_release()
            .call_concurrent(accessor, (borrow(handler),))
            .await
            .unwrap();
    }
    accessor.with(|mut a| {
        a.get()
            .observed
            .assert_pending_phase(phase, start, shortcut);
    });
}

/// Observe one closed setup phase without borrowing state across guest execution.
fn setup_phase(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    start: usize,
    phase: bool,
) {
    accessor.with(|mut a| {
        super::setup_sessions::pair_phase(&a.get().observed.transcript[start..], phase);
    });
}

/// Observe closed send-admission sets without nesting another host borrow in the driver.
fn send_pair_phase(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    start: usize,
    second_completed: bool,
) {
    accessor.with(|mut a| {
        super::replacement_commands::send_pair_phase(
            &a.get().observed.transcript[start..],
            second_completed,
        );
    });
}

/// Invokes the selected exported method with the same borrowed emitter.
pub(super) async fn component_call(
    harness: &mut Harness,
    component: Resource<super::host::Identity>,
    call: super::renderer_host::Call<'_>,
    emitter: &Resource<super::bus_host::Emitter>,
) -> Result<Result<super::renderer_host::Reply, String>, String> {
    use super::renderer_host::{Call, Reply};
    let borrow = Resource::new_borrow(emitter.rep());
    Ok(match call {
        Call::Render(width) => harness
            .exports
            .func_component_render()
            .call_async(&mut harness.store, (component, width, borrow))
            .await
            .map_err(|e| e.to_string())?
            .0
            .map(Reply::Lines),
        Call::Input(data) => harness
            .exports
            .func_component_handle_input()
            .call_async(&mut harness.store, (component, data.to_owned(), borrow))
            .await
            .map_err(|e| e.to_string())?
            .0
            .map(|()| Reply::Done),
        Call::Release => harness
            .exports
            .func_component_wants_key_release()
            .call_async(&mut harness.store, (component, borrow))
            .await
            .map_err(|e| e.to_string())?
            .0
            .map(Reply::Release),
        Call::Invalidate => harness
            .exports
            .func_component_invalidate()
            .call_async(&mut harness.store, (component, borrow))
            .await
            .map_err(|e| e.to_string())?
            .0
            .map(|()| Reply::Done),
    })
}

/// Joins teardown/cancellation to the execution's already-observed idle gate.
async fn shell_pending(
    accessor: &wasmtime::component::Accessor<super::host::State>,
    exports: &fixtures::guest::Guest,
    state: (Option<u32>, usize, Option<u32>),
    gate: (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
) {
    let (Some(handler), output, key) = state else {
        return;
    };
    let (pending, proceed) = gate;
    pending.await.unwrap();
    accessor.with(|mut a| {
        assert_eq!(a.get().observed.shell.outputs[output].chunks, [vec![122]]);
        assert_eq!(
            a.get().observed.take_registration("event user_bash"),
            Some(handler)
        );
        if let Some(key) = key {
            a.get().abort(key).unwrap();
        }
    });
    exports
        .func_release()
        .call_concurrent(accessor, (borrow(handler),))
        .await
        .unwrap();
    proceed.send(()).unwrap();
}

mod custom_footer_example;
mod message_renderer_example;
pub(crate) mod schema_qualification;

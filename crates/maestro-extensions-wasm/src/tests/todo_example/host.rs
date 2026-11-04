//! Existing callback adapters serving the shipped todo factory.
use super::super::{example_support::Adapter, host::Harness, observed::Observed};
use super::Todos;
use crate::component_adapter::{Capabilities, ToolCapabilities};
use serde_json::{Value, json};

/// Both callback adapters with the same supplied observations.
pub(super) struct Host(Adapter);
impl Host {
    /// Registers the factory on the chosen adapter.
    pub(super) fn new(component: bool) -> Result<Self, String> {
        if component {
            return Ok(Self(Adapter::Component(Box::new(Harness::example("todo")?))));
        }
        let (host, exports) =
            super::super::block_on(super::super::example_support::start::<Todos>())?;
        crate::component_adapter::install(std::rc::Rc::new(crate::component_adapter::Builder(
            host.clone(),
        )));
        Ok(Self(Adapter::Controlled(host, exports)))
    }
    /// Borrows host observations only between callback invocations.
    pub(super) fn observe<T>(&mut self, f: impl FnOnce(&mut Observed) -> T) -> T {
        self.0.observe(f)
    }
    /// Executes the registered tool, preserving its complete result.
    pub(super) fn tool(&mut self, params: &Value) -> Result<Value, String> {
        let key = self.observe(|o| o.callback("tool todo"))?;
        let result = match &mut self.0 {
            Adapter::Controlled(host, exports) => super::super::block_on(exports.invoke_tool(
                key,
                crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                    tool_call_id: "todo-call".into(),
                    params: params.to_string(),
                },
                ToolCapabilities {
                    ctx: host.context("/work"),
                    signal: None,
                    update: None,
                },
            ))?,
            Adapter::Component(host) => {
                let ctx = host.ordinary("/work").map_err(|e| e.to_string())?;
                super::super::example_support::invoke_tool_on_component(
                    host,
                    key,
                    params,
                    super::super::host_family::guest::ToolCapabilities {
                        ctx,
                        signal: None,
                        update: None,
                    },
                )?
            }
        };
        serde_json::from_str(&result).map_err(|e| e.to_string())
    }
    /// Delivers a session event with only the current branch made available.
    pub(super) fn event(&mut self, name: &str, branch: &Value) -> Result<(), String> {
        self.observe(|o| {
            o.readers.control(
                &json!({"bind":{"replies":[{"method":"getBranch","args":[null],"value":branch}]}})
                    .to_string(),
            );
        });
        let key = self.observe(|o| o.callback(&format!("event {name}")))?;
        let event = if name == "session_start" {
            json!({"type":name,"reason":"resume"})
        } else {
            json!({"type":name,"newLeafId":"leaf","oldLeafId":"old"})
        };
        match &mut self.0 {
            Adapter::Controlled(host, exports) => {
                super::super::block_on(exports.invoke_event(
                    key,
                    event.to_string(),
                    Capabilities {
                        ctx: host.context("/work"),
                        signal: None,
                    },
                ))?;
            }
            Adapter::Component(host) => {
                let caps = super::super::host_family::guest::Capabilities {
                    ctx: host.ordinary("/work").map_err(|e| e.to_string())?,
                    signal: None,
                };
                let exports = host.exports.clone();
                super::super::block_on(host.store.run_concurrent(async |accessor| {
                    exports
                        .call_invoke_event(
                            accessor,
                            super::super::host::borrow(key),
                            event.to_string(),
                            caps,
                        )
                        .await
                }))
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())??;
            }
        }
        Ok(())
    }
    /// Opens the registered list command, ignoring its unused arguments.
    pub(super) fn command(&mut self) -> Result<(), String> {
        let key = self.observe(|o| o.callback("command todos"))?;
        match &mut self.0 {
            Adapter::Controlled(host, exports) => super::super::block_on(exports.invoke_command(
                key,
                "ignored".into(),
                host.command_context("/work"),
            )),
            Adapter::Component(host) => super::super::example_support::invoke_command_on_component(
                host, key, "ignored", false,
            ),
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        match &mut self.0 {
            Adapter::Controlled(host, _) => {
                super::super::example_support::released(host);
            }
            Adapter::Component(host) => {
                super::super::example_support::released_component(host).unwrap();
            }
        }
    }
}

mod render;

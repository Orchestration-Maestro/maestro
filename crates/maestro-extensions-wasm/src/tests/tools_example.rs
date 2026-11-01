//! The shipped tool selector exercised through its registered callbacks on both adapters.
use super::{controlled::Controlled, host::Harness, observed::Observed};
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::{Value, json};

#[path = "../../../../examples/extensions/tools.rs"]
mod tools;

/// The factory exported by the shipped component.
struct Tools;
impl Extension for Tools {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        tools::factory(api)
    }
}

/// Both existing adapters, serving the same registered callbacks.
enum Host {
    /// In-process author adapter.
    Controlled(Controlled, Exports<Controlled>),
    /// Built component adapter.
    Component(Box<Harness>),
}
impl Host {
    /// Starts the shipped factory on the selected adapter.
    fn new(component: bool) -> Result<Self, String> {
        if component {
            let path = super::build::example_component("tools")?;
            Ok(Self::Component(Box::new(
                super::block_on(Harness::start(&path)).map_err(|e| e.to_string())?,
            )))
        } else {
            let (host, exports) = super::block_on(super::example_support::start::<Tools>())?;
            crate::component_adapter::install(std::rc::Rc::new(crate::component_adapter::Builder(
                host.clone(),
            )));
            Ok(Self::Controlled(host, exports))
        }
    }
    /// Supplies inventory, active selection and current-branch entries independently.
    fn supply(&mut self, branch: &Value) {
        self.observe(|o| {
            o.actions.raw = Some(json!([
                {"name":"read","description":"Read files","parameters":{},"sourceInfo":{"path":"/tools","source":"local","scope":"project","origin":"top-level"}},
                {"name":"bash","description":"Run commands","parameters":{},"sourceInfo":{"path":"/tools","source":"local","scope":"project","origin":"top-level"}},
                {"name":"extra","description":"Extension tool","parameters":{},"sourceInfo":{"path":"/tools","source":"local","scope":"project","origin":"top-level"}}
            ]).to_string());
            o.actions.value = json!(["read"]);
            o.readers.control(&json!({"bind":{"replies":[{"method":"getBranch","args":[null],"value":branch}]}}).to_string());
        });
    }
    /// Mutates or reads observations without retaining a borrow across guest calls.
    fn observe<T>(&mut self, f: impl FnOnce(&mut Observed) -> T) -> T {
        match self {
            Self::Controlled(host, _) => f(&mut host.observe()),
            Self::Component(host) => f(host.observed()),
        }
    }
    /// Runs a registered session event through the public adapter.
    fn event(&mut self, name: &str) -> Result<(), String> {
        let key = self.observe(|o| o.callback(&format!("event {name}")))?;
        let event = if name == "session_start" {
            json!({"type":name,"reason":"resume"})
        } else {
            json!({"type":name,"newLeafId":"leaf","oldLeafId":"old"})
        };
        match self {
            Self::Controlled(host, exports) => {
                let caps = Capabilities {
                    ctx: host.context("/work"),
                    signal: None,
                };
                super::block_on(exports.invoke_event(key, event.to_string(), caps))?;
            }
            Self::Component(host) => {
                let caps = super::host_family::guest::Capabilities {
                    ctx: host.ordinary("/work").map_err(|e| e.to_string())?,
                    signal: None,
                };
                let exports = host.exports.clone();
                super::block_on(host.store.run_concurrent(async |accessor| {
                    exports
                        .call_invoke_event(
                            accessor,
                            super::host::borrow(key),
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
    /// Returns only active-tool updates and persisted configuration, in order.
    fn effects(&mut self) -> Vec<String> {
        self.observe(|o| {
            o.transcript
                .iter()
                .filter(|line| {
                    line.starts_with("action setActiveTools")
                        || line.starts_with("entry tools-config")
                })
                .cloned()
                .collect()
        })
    }
}

/// A supplied custom entry on the selected branch, not the entire session tree.
fn config(id: &str, enabled: &Value) -> Value {
    json!({"type":"custom","id":id,"parentId":null,"timestamp":"2026-01-01T00:00:00Z","customType":"tools-config","data":{"enabledTools":enabled}})
}

/// Both event kinds restore only the newest configuration on the selected branch.
#[test]
fn tools_restore_current_branch_and_filter_removed_tools() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.supply(&json!([
            config("first", &json!(["read"])),
            config("last", &json!(["extra", "removed", "extra", "bash"]))
        ]));
        host.event("session_start")?;
        assert_eq!(
            host.effects(),
            [r#"action setActiveTools [["extra","bash"]]"#]
        );
        host.supply(&json!([config("empty", &json!([]))]));
        host.event("session_tree")?;
        assert_eq!(host.effects().last().unwrap(), "action setActiveTools [[]]");
        host.supply(&json!([]));
        let before = host.effects();
        host.event("session_tree")?;
        assert_eq!(
            host.effects(),
            before,
            "no saved selection leaves active tools untouched"
        );
    }
    Ok(())
}

impl Host {
    /// Opens the registered selector with terminal input supplied to the existing driver.
    fn command(&mut self) -> Result<(), String> {
        let key = self.observe(|o| o.callback("command tools"))?;
        match self {
            Self::Controlled(host, exports) => super::block_on(exports.invoke_command(
                key,
                "ignored arguments".into(),
                host.command_context("/work"),
            )),
            Self::Component(host) => super::example_support::invoke_command_on_component(
                host,
                key,
                "ignored arguments",
                false,
            ),
        }
    }
    /// Supplies literal theme answers and a deterministic custom-view script.
    fn script(&mut self, inputs: &[&str], frames: Vec<Vec<String>>) {
        self.observe(|o| {
            o.themes.theme("dark").replies.extend([
                super::theme_host::Reply {
                    method: "bold".into(),
                    operands: json!(["Tool Configuration"]),
                    answer: json!({"ok":"<bold>Tool Configuration"}),
                },
                super::theme_host::Reply {
                    method: "fg".into(),
                    operands: json!(["accent", "<bold>Tool Configuration"]),
                    answer: json!({"ok":"<accent><bold>Tool Configuration"}),
                },
            ]);
            o.ui.custom_script = Some(super::custom_host::Script {
                width: None,
                inputs: inputs.iter().map(|s| (*s).to_owned()).collect(),
                frames,
                ..super::custom_host::Script::default()
            });
        });
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let Self::Controlled(host, _) = self else {
            return;
        };
        super::example_support::release_all(
            host,
            &["command tools", "event session_start", "event session_tree"],
        );
    }
}

/// Literal expected settings rows, including the title and the selected-row styles.
fn frame(selected: usize, enabled: [&str; 3]) -> Vec<String> {
    let mut rows = vec!["<accent><bold>Tool Configuration".into(), String::new()];
    for (index, (name, value)) in ["read", "bash", "extra"]
        .into_iter()
        .zip(enabled)
        .enumerate()
    {
        rows.push(format!("<label {}>{name}", selected == index));
        rows.push(format!("<value {}>{value}", selected == index));
    }
    rows
}

/// Settings reflect all available tools, and each input applies and persists before closing.
#[test]
fn tools_selector_applies_and_persists_each_change() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.supply(&json!([]));
        host.event("session_start")?;
        assert!(host.effects().is_empty());
        host.script(
            &["\r", "\x1b[B", "\r", "\x1b[A", "\r", "\x1b"],
            vec![
                frame(0, ["enabled", "disabled", "disabled"]),
                frame(0, ["disabled", "disabled", "disabled"]),
                frame(1, ["disabled", "disabled", "disabled"]),
                frame(1, ["disabled", "enabled", "disabled"]),
                frame(0, ["disabled", "enabled", "disabled"]),
                frame(0, ["enabled", "enabled", "disabled"]),
                frame(0, ["enabled", "enabled", "disabled"]),
            ],
        );
        host.command()?;
        assert_eq!(
            host.effects(),
            [
                "action setActiveTools [[]]",
                r#"entry tools-config {"enabledTools":[]}"#,
                r#"action setActiveTools [["bash"]]"#,
                r#"entry tools-config {"enabledTools":["bash"]}"#,
                r#"action setActiveTools [["bash","read"]]"#,
                r#"entry tools-config {"enabledTools":["bash","read"]}"#,
            ]
        );
        host.observe(|o| {
            assert_eq!(
                o.registrations.commands,
                [("tools".into(), Some("Enable/disable tools".into()), false)]
            );
        });
    }
    Ok(())
}

/// The selector refreshes inventory at command time, including empty and long inventories.
#[test]
fn tools_selector_refreshes_inventory_and_limits_visible_rows() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.supply(&json!([]));
        host.event("session_start")?;
        let tool = host.observe(|o| {
            serde_json::from_str::<Value>(o.actions.raw.as_ref().unwrap()).unwrap()[2].clone()
        });
        for count in [0, 16] {
            let inventory: Vec<_> = (0..count)
                .map(|index| {
                    let mut tool = tool.clone();
                    tool["name"] = json!(format!("new-{index}"));
                    tool
                })
                .collect();
            host.observe(|o| o.actions.raw = Some(json!(inventory).to_string()));
            let mut rows = vec!["<accent><bold>Tool Configuration".into(), String::new()];
            for index in 0..count {
                rows.push(format!("<label {}>new-{index}", index == 0));
                rows.push(format!("<value {}>disabled", index == 0));
            }
            host.script(&["\x1b"], vec![rows.clone(), rows]);
            host.command()?;
            host.observe(|o| {
                assert!(o.transcript.iter().any(|line| line.contains(&format!(
                    "\"maxVisible\":{}",
                    if count == 0 { 2 } else { 15 }
                ))));
            });
        }
        assert!(
            host.effects().is_empty(),
            "closing without changes never writes configuration"
        );
    }
    Ok(())
}

/// Host failures stay visible and do not turn into successful commands.
#[test]
fn tools_selector_propagates_host_rejections() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.supply(&json!([]));
        host.event("session_start")?;
        host.observe(|o| o.actions.error = Some("inventory refused".into()));
        assert_eq!(host.command(), Err("inventory refused".into()));
        host.observe(|o| {
            o.actions.error = None;
            o.ui.reject("custom", "selector refused");
        });
        assert_eq!(host.command(), Err("selector refused".into()));
        assert!(host.effects().is_empty());
    }
    Ok(())
}

//! Presets exercised through registered callbacks on the controlled and component adapters.
use super::{controlled::Controlled, host::Harness, observed::Observed};
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::{Value, json};

#[path = "../../../../examples/extensions/preset.rs"]
mod preset;

/// The factory exported by the shipped example.
struct Preset;
impl Extension for Preset {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        preset::factory(api)
    }
}

/// Read-only fixture directory supplied to both standard libraries.
fn fixtures() -> String {
    format!("{}/src/tests/preset_data", env!("CARGO_MANIFEST_DIR"))
}

/// Replies for exact path operands and independently selected host settings.
fn observations() -> Observed {
    let mut o = Observed::default();
    o.actions.value = json!({
        "getThinkingLevel":"medium", "getActiveTools":["bash"], "setModel":true,
        "getAllTools":[
            {"name":"read","description":"Read","parameters":{},"sourceInfo":{"path":"/tools","source":"local","scope":"project","origin":"top-level"}},
            {"name":"extra","description":"Extra","parameters":{},"sourceInfo":{"path":"/tools","source":"local","scope":"project","origin":"top-level"}}
        ],
        "paths.expand_path":"/agent", "keys.ctrl_shift":"ctrl+shift+u"
    });
    for (args, file) in [
        (json!(["presets.json", "/agent"]), "global.json"),
        (json!([".maestro/presets.json", "/work"]), "project.json"),
    ] {
        o.actions.helpers.insert(
            format!("paths.resolve_to_cwd {args}"),
            json!(format!("{}/{file}", fixtures())),
        );
    }
    for name in [
        "plan",
        "alpha",
        "zeta",
        "empty",
        "invalidTools",
        "missingModel",
    ] {
        o.themes
            .theme("dark")
            .replies
            .push(super::theme_host::Reply {
                method: "fg".into(),
                operands: json!(["accent", format!("preset:{name}")]),
                answer: json!({"ok":format!("<accent>preset:{name}")}),
            });
    }
    for c in (b'!'..=b'~')
        .map(char::from)
        .chain([' ', '\n', '\t', '\u{feff}', '\u{85}'])
    {
        o.actions.helpers.insert(
            format!("is_whitespace_char {}", json!([c.to_string()])),
            json!(matches!(c, ' ' | '\n' | '\t' | '\u{feff}')),
        );
    }
    o.catalog.control(
        &json!({"registry":{"replies":[
            {"method":"find","args":["configured","custom-model"],"value":model("custom-model")},
            {"method":"find","args":["configured","absent"],"value":null}
        ]}})
        .to_string(),
    );
    o
}

/// An exact configured descriptor, not a generated catalog lookup.
fn model(id: &str) -> Value {
    json!({"id":id,"name":"Configured model","api":"test-api","provider":"configured","baseUrl":"http://unused.invalid","reasoning":true,"input":["text"],"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0},"contextWindow":4096.0,"maxTokens":1024.0})
}

/// Existing adapters serving identical callback invocations and host replies.
enum Host {
    /// Author code in process.
    Controlled(Controlled, Exports<Controlled>),
    /// Built WebAssembly component with read-only fixture access.
    Component(Box<Harness>),
}
impl Host {
    /// Starts one factory with controlled inputs before registration.
    fn new(component: bool) -> Result<Self, String> {
        if component {
            let path = super::build::example_component("preset")?;
            let (engine, component) = super::build::compile(&path).map_err(|e| e.to_string())?;
            let mut wasi = wasmtime_wasi::WasiCtxBuilder::new();
            wasi.preopened_dir(fixtures(), fixtures(), wasmtime_wasi::FsPerms::ReadOnly)
                .map_err(|e| e.to_string())?;
            Ok(Self::Component(Box::new(
                super::block_on(Harness::from_compiled_with_inputs(
                    &engine,
                    &component,
                    observations(),
                    wasi.build(),
                ))
                .map_err(|e| e.to_string())?,
            )))
        } else {
            let host = Controlled::default();
            *host.observe() = observations();
            let exports = Exports::new(host.clone());
            super::block_on(exports.start::<Preset>())?;
            crate::component_adapter::install(std::rc::Rc::new(crate::component_adapter::Builder(
                host.clone(),
            )));
            Ok(Self::Controlled(host, exports))
        }
    }
    /// Reads or supplies host observations without holding a borrow across callbacks.
    fn observe<T>(&mut self, f: impl FnOnce(&mut Observed) -> T) -> T {
        match self {
            Self::Controlled(host, _) => f(&mut host.observe()),
            Self::Component(host) => f(host.observed()),
        }
    }
    /// Supplies current-branch entries independently of the whole-session history.
    fn branch(&mut self, entries: &Value) {
        self.observe(|o| {
            o.readers.control(
                &json!({"bind":{"replies":[
                    {"method":"getBranch","args":[null],"value":entries},
                    {"method":"getEntries","args":[],"value":[saved(&json!("wrong-branch"))]}
                ]}})
                .to_string(),
            );
        });
    }
    /// Delivers a typed event through its registered handler.
    fn event(&mut self, event: &Value) -> Result<super::documents::Answer, String> {
        let key =
            self.observe(|o| o.callback(&format!("event {}", event["type"].as_str().unwrap())))?;
        match self {
            Self::Controlled(host, exports) => {
                let delivered = super::block_on(exports.invoke_event(
                    key,
                    event.to_string(),
                    Capabilities {
                        ctx: host.context("/work"),
                        signal: None,
                    },
                ))?;
                super::documents::Answer::of(super::guest_family::delivery(delivered))
            }
            Self::Component(host) => {
                let caps = super::host_family::guest::Capabilities {
                    ctx: host.ordinary("/work").map_err(|e| e.to_string())?,
                    signal: None,
                };
                let exports = host.exports.clone();
                let delivered = super::block_on(host.store.run_concurrent(async |accessor| {
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
                super::documents::Answer::of(super::host_family::delivery(delivered))
            }
        }
    }
    /// Invokes the registered slash command with literal arguments.
    fn command(&mut self, args: &str) -> Result<(), String> {
        let key = self.observe(|o| o.callback("command preset"))?;
        match self {
            Self::Controlled(host, exports) => super::block_on(exports.invoke_command(
                key,
                args.into(),
                host.command_context("/work"),
            )),
            Self::Component(host) => {
                super::example_support::invoke_command_on_component(host, key, args, false)
            }
        }
    }
    /// Notification messages and their exact severities.
    fn notifications(&mut self) -> Vec<Value> {
        self.observe(|o| {
            super::ui_host::receipts(&o.transcript)
                .into_iter()
                .filter(|r| r.method == "notify")
                .map(|r| r.operands)
                .collect()
        })
    }
    /// The admitted action operands for one host operation.
    fn actions(&mut self, method: &str) -> Vec<Value> {
        self.observe(|o| {
            o.transcript
                .iter()
                .filter_map(|line| line.strip_prefix(&format!("action {method} ")))
                .map(|text| serde_json::from_str(text).unwrap())
                .collect()
        })
    }
}

/// One persisted preset choice on the selected branch.
fn saved(name: &Value) -> Value {
    json!({"type":"custom","id":"saved","parentId":null,"timestamp":"2026-01-01T00:00:00Z","customType":"preset-state","data":{"name":name}})
}

/// Prompt input retains the assembled text exactly.
fn prompt() -> Value {
    json!({"type":"before_agent_start","prompt":"question","systemPrompt":" base \n","systemPromptOptions":{"cwd":"/work"}})
}

#[test]
fn preset_layers_restore_branch_instructions_without_reapplying_settings() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.branch(&json!([saved(&json!("zeta")), saved(&json!("plan"))]));
        host.event(&json!({"type":"session_start","reason":"resume"}))?;
        let result = host.event(&prompt())?;
        assert_eq!(
            result.ended,
            super::documents::Ended::Returned(Some(
                json!({"systemPrompt":" base \n\n\nProject instructions with more than thirty characters."})
            ))
        );
        assert!(host.actions("setModel").is_empty());
        assert!(host.actions("setThinkingLevel").is_empty());
        assert!(host.actions("setActiveTools").is_empty());
        let statuses = host.observe(|o| {
            super::ui_host::receipts(&o.transcript)
                .into_iter()
                .filter(|r| r.method == "setStatus")
                .map(|r| r.operands)
                .collect::<Vec<_>>()
        });
        assert_eq!(statuses, [json!(["preset", "<accent>preset:plan"])]);
        host.branch(&json!([]));
        host.event(&json!({"type":"session_start","reason":"resume"}))?;
        assert!(
            host.event(&prompt())?.ended == super::documents::Ended::Returned(None),
            "a new branch must not retain the previous preset"
        );
    }
    Ok(())
}

impl Host {
    /// Checks flag, command and shortcut declarations at the registration seam.
    fn assert_registrations(&mut self) {
        self.observe(|o| {
            assert_eq!(o.registrations.commands,[("preset".into(),Some("Switch preset configuration".into()),false)]);
            assert_eq!(o.registrations.shortcuts,[("ctrl+shift+u".into(),Some("Cycle presets".into()))]);
            assert_eq!(o.registrations.flags,[("preset".into(),json!({"description":"Preset configuration to use","type":"string","default":null}))]);
        });
    }
    /// Runs a successful session event and asserts its lack of a decision result.
    fn session(&mut self, kind: &str) -> Result<(), String> {
        let event = match kind {
            "session_start" => json!({"type":kind,"reason":"startup"}),
            "session_shutdown" => json!({"type":kind,"reason":"quit"}),
            "session_tree" => json!({"type":kind,"newLeafId":"next","oldLeafId":"old"}),
            _ => json!({"type":kind,"turnIndex":0.0,"timestamp":0.0}),
        };
        assert_eq!(
            self.event(&event)?,
            super::documents::Answer::returned(&event, None)
        );
        Ok(())
    }
    /// Supplies a different file for either configuration layer, without writing project files.
    fn file(&mut self, global: bool, file: &str) {
        let args = if global {
            json!(["presets.json", "/agent"])
        } else {
            json!([".maestro/presets.json", "/work"])
        };
        self.observe(|o| {
            o.actions.helpers.insert(
                format!("paths.resolve_to_cwd {args}"),
                json!(format!("{}/{file}", fixtures())),
            );
        });
    }
    /// Invokes the configured keyboard shortcut through the public adapter.
    fn cycle(&mut self) -> Result<(), String> {
        let key = self.observe(|o| o.callback("shortcut ctrl+shift+u"))?;
        match self {
            Self::Controlled(host, exports) => {
                super::block_on(exports.invoke_shortcut(key, host.context("/work")))
            }
            Self::Component(host) => {
                let context = host.ordinary("/work").map_err(|e| e.to_string())?;
                let exports = host.exports.clone();
                super::block_on(host.store.run_concurrent(async |accessor| {
                    exports
                        .call_invoke_shortcut(accessor, super::host::borrow(key), context)
                        .await
                }))
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?
            }
        }
    }
}

#[test]
fn preset_applies_configured_models_valid_tools_and_shared_whitespace() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.branch(&json!([]));
        host.session("session_start")?;
        host.assert_registrations();
        host.session("turn_start")?;
        assert!(host.observe(|o| {
            o.transcript
                .iter()
                .all(|line| !line.starts_with("entry preset-state "))
        }));
        host.command("\u{feff} plan \u{feff}")?;
        assert_eq!(host.actions("setModel"), [json!([model("custom-model")])]);
        assert_eq!(host.actions("setThinkingLevel"), [json!(["high"])]);
        assert_eq!(host.actions("setActiveTools"), [json!([["read", "extra"]])]);
        assert_eq!(
            host.notifications(),
            [
                json!(["Preset \"plan\": Unknown tools: missing", "Warning"]),
                json!(["Preset \"plan\" activated", "Info"])
            ]
        );
        host.command("\u{85}plan\u{85}")?;
        assert_eq!(
            host.notifications().last().unwrap(),
            &json!([
                "Unknown preset \"\u{85}plan\u{85}\". Available: zeta, plan, empty, invalidTools, missingModel, alpha",
                "Error"
            ])
        );
        host.command("missingModel")?;
        assert!(host.notifications().contains(&json!([
            "Preset \"missingModel\": Model configured/absent not found",
            "Warning"
        ])));
        host.observe(|o| o.actions.value["setModel"] = json!(false));
        host.command("plan")?;
        assert!(host.notifications().contains(&json!([
            "Preset \"plan\": No API key for configured/custom-model",
            "Warning"
        ])));
        let tools = host.actions("setActiveTools");
        host.command("invalidTools")?;
        host.command("alpha")?;
        host.command("empty")?;
        assert_eq!(
            host.actions("setActiveTools"),
            tools,
            "empty or wholly invalid lists do not disable tools"
        );
        assert_eq!(
            host.actions("setThinkingLevel").last().unwrap(),
            &json!(["off"])
        );
        host.observe(|o| o.actions.error = Some("host refused".into()));
        assert_eq!(host.command("plan"), Err("host refused".into()));
    }
    Ok(())
}

#[test]
fn preset_cycles_restore_original_settings_and_persist_cleared_branch_state() -> Result<(), String>
{
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.branch(&json!([]));
        host.observe(|o|o.catalog.control(&json!({"model":{"replies":[{"method":"model","args":[],"value":model("original")}]}}).to_string()));
        host.session("session_start")?;
        host.command("plan")?;
        host.session("turn_start")?;
        host.cycle()?; // plan -> zeta, using sorted order rather than file order.
        host.cycle()?; // zeta -> none, restoring the first snapshot.
        assert_eq!(
            host.actions("setModel").last().unwrap(),
            &json!([model("original")])
        );
        assert_eq!(
            host.actions("setThinkingLevel").last().unwrap(),
            &json!(["medium"])
        );
        assert_eq!(
            host.actions("setActiveTools").last().unwrap(),
            &json!([["bash"]])
        );
        assert_eq!(
            host.notifications().last().unwrap(),
            &json!(["Preset cleared, defaults restored", "Info"])
        );
        host.session("turn_start")?;
        let entries = host.observe(|o| {
            o.transcript
                .iter()
                .filter(|line| line.starts_with("entry preset-state "))
                .cloned()
                .collect::<Vec<_>>()
        });
        assert_eq!(
            entries,
            [
                "entry preset-state {\"name\":\"plan\"}",
                "entry preset-state {\"name\":null}"
            ]
        );
        host.branch(&json!([saved(&json!("plan")), saved(&Value::Null)]));
        host.session("session_tree")?;
        assert_eq!(
            host.event(&prompt())?.ended,
            super::documents::Ended::Returned(None)
        );
        host.cycle()?; // none -> alpha.
        assert_eq!(
            host.notifications().last().unwrap(),
            &json!(["Preset \"alpha\" activated", "Info"])
        );
    }
    Ok(())
}

#[test]
fn preset_rejects_invalid_layers_without_losing_the_other_file() -> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.branch(&json!([]));
        for bad in ["invalid.json", "malformed.json", "absent.json"] {
            host.file(false, bad);
            host.session("session_start")?;
            host.command("plan")?;
            assert_eq!(
                host.event(&prompt())?.ended,
                super::documents::Ended::Returned(Some(
                    json!({"systemPrompt":" base \n\n\nglobal plan"})
                ))
            );
        }
        host.file(true, "malformed.json");
        host.file(false, "project.json");
        host.session("session_start")?;
        host.command("alpha")?;
        assert_eq!(
            host.event(&prompt())?.ended,
            super::documents::Ended::Returned(Some(json!({"systemPrompt":" base \n\n\nAlpha"})))
        );
        host.file(false, "absent.json");
        host.session("session_start")?;
        host.command(" ")?;
        host.cycle()?;
        let warning = json!([
            "No presets defined. Add presets to ~/.maestro/agent/presets.json or .maestro/presets.json",
            "Warning"
        ]);
        assert_eq!(
            &host.notifications()[host.notifications().len() - 2..],
            &[warning.clone(), warning]
        );
        host.command("unknown")?;
        assert_eq!(
            host.notifications().last().unwrap(),
            &json!([
                "Unknown preset \"unknown\". Available: (none defined)",
                "Error"
            ])
        );
    }
    Ok(())
}

#[test]
fn preset_startup_flag_is_applied_before_returning_and_overrides_saved_name() -> Result<(), String>
{
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.branch(&json!([saved(&json!("zeta"))]));
        host.observe(|o| {
            o.registrations
                .values
                .insert("preset".into(), Ok(Some(json!("plan"))));
        });
        host.session("session_start")?;
        assert_eq!(host.actions("setModel"), [json!([model("custom-model")])]);
        assert_eq!(host.actions("setThinkingLevel"), [json!(["high"])]);
        assert_eq!(host.actions("setActiveTools"), [json!([["read", "extra"]])]);
        assert_eq!(
            host.event(&prompt())?.ended,
            super::documents::Ended::Returned(Some(
                json!({"systemPrompt":" base \n\n\nProject instructions with more than thirty characters."})
            ))
        );
        assert!(
            host.notifications()
                .contains(&json!(["Preset \"plan\" activated", "Info"]))
        );
        host.observe(|o| {
            o.registrations
                .values
                .insert("preset".into(), Ok(Some(json!("unknown"))));
        });
        host.session("session_start")?;
        assert_eq!(
            host.event(&prompt())?.ended,
            super::documents::Ended::Returned(None)
        );
        assert_eq!(
            host.notifications().last().unwrap(),
            &json!([
                "Unknown preset \"unknown\". Available: zeta, plan, empty, invalidTools, missingModel, alpha",
                "Warning"
            ])
        );
    }
    Ok(())
}

/// Literal retained-selector frame for each selected row.
fn selector_frame(selected: usize, active: bool) -> Vec<String> {
    let names = [
        "zeta",
        if active { "plan (active)" } else { "plan" },
        "empty",
        "invalidTools",
        "missingModel",
        "alpha",
        "(none)",
    ];
    let mut frame = vec![
        "<accent>────────────────────".into(),
        "text <accent><bold>Select Preset 0 0|20".into(),
    ];
    for (index, name) in names.into_iter().enumerate() {
        frame.push(if index == selected {
            format!("<accent>> {name}")
        } else {
            format!("  {name}")
        });
    }
    frame.extend([
        "text <dim>↑↓ navigate • enter select • esc cancel 0 0|20".into(),
        "<accent>────────────────────".into(),
    ]);
    frame
}

impl Host {
    /// Supplies theme answers and literal terminal frames, including cancellation.
    fn selector(&mut self, inputs: &[&str], selected: &[usize], active: bool) {
        self.observe(|o| {
            for (method, args, answer) in [
                ("bold", json!(["Select Preset"]), "<bold>Select Preset"),
                (
                    "fg",
                    json!(["accent", "<bold>Select Preset"]),
                    "<accent><bold>Select Preset",
                ),
                (
                    "fg",
                    json!(["dim", "↑↓ navigate • enter select • esc cancel"]),
                    "<dim>↑↓ navigate • enter select • esc cancel",
                ),
                (
                    "fg",
                    json!(["accent", "────────────────────"]),
                    "<accent>────────────────────",
                ),
            ] {
                o.themes
                    .theme("dark")
                    .replies
                    .push(super::theme_host::Reply {
                        method: method.into(),
                        operands: args,
                        answer: json!({"ok":answer}),
                    });
            }
            for name in [
                "zeta",
                "plan",
                "plan (active)",
                "empty",
                "invalidTools",
                "missingModel",
                "alpha",
                "(none)",
            ] {
                o.themes
                    .theme("dark")
                    .replies
                    .push(super::theme_host::Reply {
                        method: "fg".into(),
                        operands: json!(["accent", format!("> {name}")]),
                        answer: json!({"ok":format!("<accent>> {name}")}),
                    });
            }
            o.ui.custom_script = Some(super::custom_host::Script {
                width: None,
                inputs: inputs.iter().map(|s| (*s).into()).collect(),
                frames: selected
                    .iter()
                    .map(|index| selector_frame(*index, active))
                    .collect(),
                ..Default::default()
            });
        });
    }
    /// Reads the host's retained list choices, not this example's private state.
    fn choices(&mut self) -> Vec<crate::SelectItem> {
        self.observe(|o| {
            let key = o
                .transcript
                .iter()
                .rev()
                .find_map(|line| {
                    let (prefix, _) = line.split_once(" new select ")?;
                    prefix.strip_prefix("widget ")?.parse::<u32>().ok()
                })
                .unwrap();
            super::widget_lists::list(o, key).unwrap().items.clone()
        })
    }
}

#[test]
fn preset_selector_renders_descriptions_marks_active_and_restores_fallback_tools()
-> Result<(), String> {
    for component in [false, true] {
        let mut host = Host::new(component)?;
        host.branch(&json!([saved(&json!("plan"))]));
        host.session("session_start")?;
        host.selector(&["\x1b"], &[0, 0], true);
        host.command("")?;
        let choices = host.choices();
        assert_eq!(choices[1].label, "plan (active)");
        assert_eq!(
            choices[1].description.as_deref(),
            Some(
                "configured/custom-model | thinking:high | tools:read,missing,extra | \"Project instructions with m...\""
            )
        );
        assert_eq!(
            choices[5].description.as_deref(),
            Some("thinking:off | tools: | \"Alpha\"")
        );
        assert_eq!(
            choices[6].description.as_deref(),
            Some("Clear active preset, restore defaults")
        );
        assert!(
            host.notifications().is_empty(),
            "cancel leaves the current preset alone"
        );
        host.selector(&["\x1b[A", "\r"], &[0, 6, 6], true);
        host.command("")?;
        assert_eq!(
            host.actions("setActiveTools"),
            [json!([["read", "bash", "edit", "write"]])]
        );
        assert_eq!(
            host.notifications().last().unwrap(),
            &json!(["Preset cleared, defaults restored", "Info"])
        );
        host.selector(&["\r"], &[0, 0], false);
        host.command("")?;
        assert_eq!(
            host.notifications().last().unwrap(),
            &json!(["Preset \"zeta\" activated", "Info"])
        );
    }
    Ok(())
}

#[test]
fn preset_startup_waits_for_pending_model_before_the_first_turn() -> Result<(), String> {
    let mut host = Host::new(false)?;
    host.branch(&json!([]));
    host.observe(|o| {
        o.registrations
            .values
            .insert("preset".into(), Ok(Some(json!("plan"))));
    });
    let key = host.observe(|o| o.callback("event session_start"))?;
    let Host::Controlled(controlled, _) = &host else {
        unreachable!();
    };
    let exports = Exports::new(controlled.clone());
    let caps = Capabilities {
        ctx: controlled.context("/work"),
        signal: None,
    };
    let (hold, started, open) = super::observed::gate();
    host.observe(|o| o.hold_next_idle(hold));
    let mut future = Box::pin(exports.invoke_event(
        key,
        json!({"type":"session_start","reason":"startup"}).to_string(),
        caps,
    ));
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    assert!(
        std::future::Future::poll(future.as_mut(), &mut cx).is_pending(),
        "startup cannot complete while preset model selection is pending"
    );
    super::block_on(started).map_err(|e| e.to_string())?;
    assert!(host.actions("setModel").is_empty());
    assert!(host.actions("setThinkingLevel").is_empty());
    assert!(host.actions("setActiveTools").is_empty());
    open.send(()).map_err(|()| "model selection was dropped")?;
    let delivered = super::block_on(future)?;
    assert_eq!(
        super::documents::Answer::of(super::guest_family::delivery(delivered))?.ended,
        super::documents::Ended::Returned(None)
    );
    assert_eq!(host.actions("setModel"), [json!([model("custom-model")])]);
    assert_eq!(host.actions("setThinkingLevel"), [json!(["high"])]);
    assert_eq!(host.actions("setActiveTools"), [json!([["read", "extra"]])]);
    assert_eq!(
        host.event(&prompt())?.ended,
        super::documents::Ended::Returned(Some(
            json!({"systemPrompt":" base \n\n\nProject instructions with more than thirty characters."})
        ))
    );
    host.session("turn_start")?;
    assert!(host.observe(|o| {
        o.transcript
            .contains(&"entry preset-state {\"name\":\"plan\"}".into())
    }));
    Ok(())
}

impl Drop for Host {
    fn drop(&mut self) {
        let Self::Controlled(host, _) = self else {
            return;
        };
        let handlers = host.observe().ui.lists.take_handlers();
        for key in handlers {
            crate::component_adapter::release(key);
        }
        super::example_support::released(host);
        let styles = host.observed().live_identities();
        for key in styles {
            crate::component_adapter::release(key);
        }
        if !std::thread::panicking() {
            assert_eq!(
                host.observed().live,
                0,
                "all borrowed capabilities released"
            );
        }
    }
}

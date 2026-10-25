//! Controlled component callbacks replayed against independently recorded output.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../../maestro-test-terminal/src/manual_runtime.rs"]
#[allow(dead_code)]
mod manual_runtime;
use maestro_export::{
    ToolHtmlRenderer, ToolHtmlRendererDeps, ToolHtmlResultInput, create_tool_html_renderer,
};
use maestro_models::{DiagnosticErrorInfo, Tool};
use maestro_theme::{
    ColorMode, LiveTheme, SyntaxHighlighter, SyntaxSpan, Theme, ThemeDirectories, ThemeOptions,
    ThemeState,
};
use maestro_tools::types::local::ToolDefinition;
use maestro_tools::{ToolRenderContext, ToolRenderEnvironment};
use maestro_tui::{
    TerminalImage,
    tui::{Component, ComponentHandle},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    test: String,
    input: Config,
    expected: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    width: Option<usize>,
    cwd: Option<String>,
    #[serde(default)]
    reenter: bool,
    steps: Vec<Value>,
}
pub struct NoLanguages;
impl SyntaxHighlighter for NoLanguages {
    fn supports_language(&self, _: &str) -> bool {
        false
    }
    fn highlight(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Vec::new())
    }
}
pub fn theme() -> Theme {
    Theme::new([], [], ColorMode::Color256, ThemeOptions::default()).unwrap()
}
pub fn environment() -> Rc<ToolRenderEnvironment> {
    environment_with(Rc::new(NoLanguages))
}
pub fn environment_with(highlighter: Rc<dyn SyntaxHighlighter>) -> Rc<ToolRenderEnvironment> {
    let state = ThemeState::new(ThemeDirectories {
        themes_dir: String::new(),
        custom_themes_dir: String::new(),
    });
    Rc::new(ToolRenderEnvironment {
        runtime: Rc::new(manual_runtime::ManualRuntime::default()),
        readme_path: "/opt/maestro/README.md".into(),
        live_theme: state.theme(),
        highlighter,
        terminal_image: TerminalImage::default(),
    })
}
pub fn failure() -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: None,
        message: "controlled failure".into(),
        stack: None,
        code: None,
    }
}
pub fn definition<C, P, D, S>() -> ToolDefinition<C, P, D, S> {
    ToolDefinition {
        definition: Tool {
            name: "custom".into(),
            description: String::new(),
            parameters: json!({}).into(),
        },
        label: "custom".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| Box::pin(async { Err(failure()) })),
        render_call: None,
        render_result: None,
    }
}
/// Observes an optional value with its presence.
pub fn presence(value: Option<&Value>) -> Value {
    value.map_or_else(
        || json!({"presence":"missing"}),
        |v| json!({"presence":"present","value":v}),
    )
}
/// A component rendering one fixed line.
pub struct Line(pub String);
impl Component for Line {
    fn render(&self, _: usize) -> Vec<String> {
        vec![self.0.clone()]
    }
}
#[derive(Default)]
struct Observations {
    events: RefCell<Vec<Value>>,
    step: RefCell<Value>,
    serial: Cell<usize>,
    lookups: Cell<usize>,
    states: RefCell<HashMap<String, Rc<RefCell<Value>>>>,
}
struct RecordedComponent {
    name: String,
    phase: String,
    lines: Vec<String>,
    fail: bool,
    observations: Rc<Observations>,
}
impl Component for RecordedComponent {
    fn render(&self, _: usize) -> Vec<String> {
        panic!("fallible dispatch required")
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.observations.events.borrow_mut().push(
            json!({"phase":format!("{}_render",self.phase),"component":self.name,"width":width}),
        );
        if self.fail {
            Err(std::io::Error::other("component failure").into())
        } else {
            Ok(self.lines.clone())
        }
    }
    fn invalidate(&self) {
        panic!("component invalidation is not called")
    }
}
fn context(observations: &Observations, ctx: &ToolRenderContext<'_>, phase: &str) -> Value {
    let state = observations
        .states
        .borrow_mut()
        .entry(ctx.tool_call_id.clone())
        .or_insert_with(|| Rc::clone(&ctx.state))
        .clone();
    let before = ctx
        .state
        .borrow()
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ctx.state.borrow_mut()["count"] = json!(before + 1);
    (ctx.invalidate)();
    let last = ctx.last_component.as_ref().map(|c| {
        let c = c.as_ref() as &dyn std::any::Any;
        c.downcast_ref::<RecordedComponent>().unwrap().name.clone()
    });
    json!({"phase":phase,"id":ctx.tool_call_id,"args":presence(ctx.args),"last":last,
        "same_state":Rc::ptr_eq(&state,&ctx.state),"count_before":before,"cwd":ctx.cwd,
        "execution_started":ctx.phase.execution_started,"args_complete":ctx.phase.args_complete,
        "is_partial":ctx.phase.is_partial,"expanded":ctx.expanded,"show_images":ctx.show_images,"is_error":ctx.is_error})
}
fn component(
    observations: &Rc<Observations>,
    phase: &str,
    ctx: &ToolRenderContext<'_>,
) -> ComponentHandle {
    let serial = observations.serial.get() + 1;
    observations.serial.set(serial);
    let step = observations.step.borrow();
    if step["reuse"] == true
        && let Some(previous) = &ctx.last_component
    {
        return Rc::clone(previous);
    }
    let lines = step.get(phase).map_or_else(
        || vec![phase.into()],
        |v| serde_json::from_value(v.clone()).unwrap(),
    );
    Rc::new(RecordedComponent {
        name: format!("{phase}-{serial}"),
        phase: phase.into(),
        lines,
        fail: step["fail"] == format!("{phase}_render"),
        observations: Rc::clone(observations),
    })
}
pub fn check(group: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/component_rendering.json")).unwrap();
    let selected: Vec<_> = cases.into_iter().filter(|c| c.test == group).collect();
    assert!(!selected.is_empty());
    for case in selected {
        assert_eq!(observe(&case.input), case.expected, "{}", case.id);
    }
}
fn observe(config: &Config) -> Value {
    let observations = Rc::new(Observations::default());
    let theme = Rc::new(theme());
    let tool = observed_tool(&observations, &theme);
    let renderer_slot = Rc::new(RefCell::new(None::<Weak<dyn ToolHtmlRenderer + '_>>));
    let slot = Rc::clone(&renderer_slot);
    let reentered = Cell::new(false);
    let seen = Rc::clone(&observations);
    let lookup = |name: &str| {
        let count = seen.lookups.get() + 1;
        seen.lookups.set(count);
        seen.events
            .borrow_mut()
            .push(json!({"phase":"lookup","name":name,"count":count}));
        if config.reenter && !reentered.replace(true) {
            let renderer = slot.borrow().as_ref().unwrap().upgrade().unwrap();
            let output = renderer
                .render_call("nested", "inner", Some(&json!({})))
                .unwrap();
            seen.events
                .borrow_mut()
                .push(json!({"phase":"nested_output","output":output}));
        }
        let step = seen.step.borrow();
        if step["fail"] == "lookup" {
            return Err(failure());
        }
        let found = match step["lookup"].as_str() {
            Some("missing") => None,
            Some("call_only") => {
                let mut d: ToolDefinition = definition();
                d.render_call = tool.render_call.clone();
                Some(Rc::new(d))
            }
            Some("result_only") => {
                let mut d: ToolDefinition = definition();
                d.render_result = tool.render_result.clone();
                Some(Rc::new(d))
            }
            _ => Some(Rc::clone(&tool)),
        };
        Ok(found.map(|tool| maestro_tools::ToolRendererBinding::from_definition(&tool)))
    };
    let renderer: Rc<dyn ToolHtmlRenderer + '_> =
        Rc::new(create_tool_html_renderer(ToolHtmlRendererDeps {
            get_tool_definition: lookup,
            theme: LiveTheme::from_theme(Rc::clone(&theme)),
            cwd: config.cwd.as_deref().unwrap_or("/work"),
            width: config.width,
            environment: environment(),
        }));
    *renderer_slot.borrow_mut() = Some(Rc::downgrade(&renderer));
    outputs(config, &*renderer, &observations)
}

/// Observes the produced collapsed and expanded HTML.
pub fn result_value(result: maestro_export::ToolHtmlResult) -> Value {
    let mut value = json!({});
    if let Some(html) = result.collapsed {
        value["collapsed"] = json!(html);
    }
    if let Some(html) = result.expanded {
        value["expanded"] = json!(html);
    }
    value
}

fn observed_tool(observations: &Rc<Observations>, theme: &Rc<Theme>) -> Rc<ToolDefinition> {
    let seen = Rc::clone(observations);
    let supplied = Rc::clone(theme);
    let mut tool: ToolDefinition = definition();
    tool.render_call = Some(Rc::new(move |args, theme, ctx| {
        let mut event = context(&seen, ctx, "call_callback");
        event["direct_args"] = presence(args);
        event["same_theme"] = json!(
            theme
                .get()
                .is_ok_and(|resolved| Rc::ptr_eq(&resolved, &supplied))
        );
        seen.events.borrow_mut().push(event);
        if seen.step.borrow()["fail"] == "call_callback" {
            return Err(failure());
        }
        Ok(component(&seen, "call", ctx))
    }));
    let seen = Rc::clone(observations);
    let supplied = Rc::clone(theme);
    tool.render_result = Some(
        maestro_tools::ToolRenderResult::<_, _, maestro_models::Local>::Typed(Rc::new(
            move |result, options, theme, ctx| {
                let phase = if options.expanded {
                    "expanded"
                } else {
                    "collapsed"
                };
                let mut event = context(&seen, ctx, &format!("{phase}_callback"));
                event["content"] = presence(Some(&serde_json::to_value(result.content).unwrap()));
                event["details"] = presence(result.details);
                event["result_error"] = json!(result.is_error.unwrap());
                event["options"] =
                    json!({"expanded":options.expanded,"isPartial":options.is_partial});
                event["same_theme"] = json!(
                    theme
                        .get()
                        .is_ok_and(|resolved| Rc::ptr_eq(&resolved, &supplied))
                );
                seen.events.borrow_mut().push(event);
                if seen.step.borrow()["fail"] == format!("{phase}_callback") {
                    return Err(failure());
                }
                Ok(component(&seen, phase, ctx))
            },
        )),
    );
    Rc::new(tool)
}

fn outputs(config: &Config, renderer: &dyn ToolHtmlRenderer, observations: &Observations) -> Value {
    let mut outputs = Vec::new();
    for step in &config.steps {
        assert!(
            step.as_object()
                .unwrap()
                .keys()
                .all(|k| STEP_MEMBERS.contains(&k.as_str()))
        );
        *observations.step.borrow_mut() = step.clone();
        let id = step["id"].as_str().unwrap_or("id");
        let name = step["name"].as_str().unwrap_or("custom");
        let output = match step["op"].as_str().unwrap() {
            "call" => renderer
                .render_call(id, name, step.get("args"))
                .unwrap()
                .map(Value::String),
            "result" => {
                let empty = json!([]);
                renderer
                    .render_result(
                        id,
                        name,
                        ToolHtmlResultInput {
                            content: Some(step.get("content").unwrap_or(&empty)),
                            details: step.get("details"),
                            is_error: step["is_error"].as_bool().unwrap_or(false),
                        },
                    )
                    .unwrap()
                    .map(result_value)
            }
            "mutate" => {
                observations.states.borrow()[id].borrow_mut()["count"] = step["count"].clone();
                Some(json!("mutated"))
            }
            _ => panic!("unknown operation"),
        };
        outputs.push(presence(output.as_ref()));
    }
    json!({"outputs":outputs,"events":*observations.events.borrow()})
}

const STEP_MEMBERS: [&str; 14] = [
    "op",
    "id",
    "name",
    "args",
    "content",
    "details",
    "is_error",
    "call",
    "collapsed",
    "expanded",
    "fail",
    "lookup",
    "reuse",
    "count",
];

#[allow(dead_code)]
pub mod bindings;

//! Replacement, fallible-child propagation and weak teardown witnesses.
use maestro_app::{
    ToolRenderResultCallback, ToolRenderShell, ToolRendererBinding, ToolRendererState,
};
use maestro_chat::{ToolExecutionComponent, ToolExecutionResult};
use maestro_tui::{Component, Container};
use serde::Deserialize;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
/// Recorded replacement inputs and independently corrected image payloads.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    input: Replacement,
    expected: Vec<String>,
}
/// Complete image records, including MIME changes with identical bytes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    first: maestro_app::UserBlock,
    replacement: maestro_app::UserBlock,
}
/// Extract actual wire payloads in order, retaining repeated images.
fn payloads(rows: &[String]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| {
            row.split_once("\x1b_G").map(|(_, wire)| {
                wire.split_once(';')
                    .unwrap()
                    .1
                    .split_once("\x1b\\")
                    .unwrap()
                    .0
                    .to_owned()
            })
        })
        .collect()
}
/// Check all three real stale-cache corrections through consecutive updates.
pub fn replacement() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/image_replacement.json")).unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let input = serde_json::from_value(
            serde_json::json!({"protocol":"kitty","args":{},"definition":{}}),
        )
        .unwrap();
        let runtime = super::manual_runtime::ManualRuntime::new();
        let (params, _) = super::params(&input, &runtime);
        let row = ToolExecutionComponent::new(params).unwrap();
        row.update_result(
            ToolExecutionResult {
                content: vec![case.input.first],
                details: None,
                is_error: false,
            },
            false,
        )
        .unwrap();
        assert_eq!(
            payloads(&row.try_render(40).unwrap()).len(),
            1,
            "first {}",
            case.name
        );
        row.update_result(
            ToolExecutionResult {
                content: vec![case.input.replacement],
                details: None,
                is_error: false,
            },
            false,
        )
        .unwrap();
        let current = row.try_render(40).unwrap();
        assert_eq!(
            payloads(&current),
            case.expected,
            "replacement {}",
            case.name
        );
    }
}
/// A native child error whose shared identity survives propagation.
#[derive(Debug)]
struct Failure(Rc<()>);
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("child refused")
    }
}
impl std::error::Error for Failure {}
/// Returns normal rows from the infallible interface and the original failure otherwise.
struct Fallible(Rc<()>);
impl Component for Fallible {
    fn render(&self, _: usize) -> Vec<String> {
        vec!["child rows".into()]
    }
    fn try_render(&self, _: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Err(Box::new(Failure(self.0.clone())))
    }
    fn invalidate(&self) {}
}
/// Both retained renderer children and inherited outer children forward rendering failures.
pub fn child_error() {
    for outer in [false, true] {
        let runtime = super::manual_runtime::ManualRuntime::new();
        let (mut params, _) = super::params(&super::default_input(), &runtime);
        let marker = Rc::new(());
        let child = Rc::new(Fallible(marker.clone()));
        if !outer {
            params.tool_definition = Some(ToolRendererBinding {
                render_call: Some(Rc::new(move |_, _, _| Ok(child.clone()))),
                render_result: None,
                render_shell: Some(ToolRenderShell::Default),
            });
        }
        let row = ToolExecutionComponent::new(params).unwrap();
        if outer {
            row.add_child(Rc::new(Fallible(marker.clone())));
        }
        assert!(row.render(40).iter().any(|row| row.contains("child rows")));
        let error = row.try_render(40).unwrap_err();
        let error = error.downcast_ref::<Failure>().unwrap();
        assert!(Rc::ptr_eq(&error.0, &marker));
    }
}
/// Drop a containing writer and row while an invalidator survives outside both.
pub fn teardown() {
    let runtime = super::manual_runtime::ManualRuntime::new();
    let (mut params, _) = super::params(&super::default_input(), &runtime);
    let terminal = super::native::terminal();
    let ui = maestro_tui::TUI::new(
        terminal.handle(),
        runtime.handle(),
        params.environment.terminal_image.clone(),
        Some(false),
    );
    params.request_render = ui.weak_render_request();
    let writer_capture = Rc::new(());
    let weak = Rc::downgrade(&writer_capture);
    let invalidator = Rc::new(RefCell::new(None));
    let saved = invalidator.clone();
    params.tool_definition = Some(ToolRendererBinding {
        render_call: Some(Rc::new(move |_, _, context| {
            *saved.borrow_mut() = Some(context.invalidate.clone());
            let state = context.state.borrow().state::<Option<Rc<()>>>();
            *state.borrow_mut() = Some(writer_capture.clone());
            Ok(Rc::new(Container::new()))
        })),
        render_result: None,
        render_shell: None,
    });
    let row = Rc::new(ToolExecutionComponent::new(params).unwrap());
    ui.add_child(row.clone());
    drop(row);
    assert!(weak.upgrade().is_some());
    drop(ui);
    assert!(weak.upgrade().is_none());
    let callback = invalidator.borrow().as_ref().unwrap().clone();
    callback();
    assert_eq!(runtime.pending(), 0);
    assert_eq!(runtime.futures(), 0);
    assert!(terminal.writes().is_empty());
}

/// A result callback sees the call child already installed and the newly published theme.
pub fn live_callback_phases() {
    let input = super::default_input();
    let corpus: super::records::Corpus =
        serde_json::from_str(include_str!("../fixtures/tool_rows.json")).unwrap();
    let (publication, live) = super::published::published(&super::theme(&corpus, false)).unwrap();
    let runtime = super::manual_runtime::ManualRuntime::new();
    let (mut params, _) = super::constructor(&input, &runtime, live);
    let changed = super::theme(&corpus, true);
    let handle: Rc<RefCell<Option<std::rc::Weak<ToolExecutionComponent>>>> = Rc::default();
    let observed = handle.clone();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let observed_errors = errors.clone();
    let result = installed_call_result(observed, observed_errors);
    params.tool_definition = Some(ToolRendererBinding {
        render_call: Some(Rc::new(move |_, _, _| {
            publication.set_theme_instance(changed.clone()).unwrap();
            let text = maestro_tui::Text::new("installed call".into(), 0, 0, None);
            Ok(Rc::new(text))
        })),
        render_result: Some(result.into()),
        render_shell: None,
    });
    let row = Rc::new(ToolExecutionComponent::new(params).unwrap());
    *handle.borrow_mut() = Some(Rc::downgrade(&row));
    for is_error in [false, true] {
        row.update_result(
            ToolExecutionResult {
                content: vec![],
                details: None,
                is_error,
            },
            false,
        )
        .unwrap();
        assert!(
            row.try_render(40)
                .unwrap()
                .iter()
                .any(|line| line.contains("new theme"))
        );
    }
    assert_eq!(*errors.borrow(), [false, true]);
}

/// Three consecutive replacements finish before a controlled runtime drain.
pub fn no_deferred_publication() {
    let mut cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/image_replacement.json")).unwrap();
    let invalid = cases.remove(0).input.replacement;
    let fresh = cases.remove(0);
    let input =
        serde_json::from_value(serde_json::json!({"protocol":"kitty","args":{},"definition":{}}))
            .unwrap();
    let runtime = super::manual_runtime::ManualRuntime::new();
    let (params, requests) = super::params(&input, &runtime);
    let row = ToolExecutionComponent::new(params).unwrap();
    for block in [fresh.input.first, invalid, fresh.input.replacement] {
        row.update_result(
            ToolExecutionResult {
                content: vec![block],
                details: None,
                is_error: false,
            },
            false,
        )
        .unwrap();
    }
    let current = row.try_render(40).unwrap();
    assert_eq!(payloads(&current), fresh.expected);
    let before = requests.get();
    assert_eq!(runtime.futures(), 0);
    assert!(runtime.drain_futures().is_empty());
    runtime.settle().unwrap();
    assert_eq!(row.try_render(40).unwrap(), current);
    assert_eq!(requests.get(), before);
}

/// An infallible invalidation retains native formatting failure until a successful rebuild.
pub fn invalidation_error() {
    let corpus: super::records::Corpus =
        serde_json::from_str(include_str!("../fixtures/tool_rows.json")).unwrap();
    let valid = super::theme(&corpus, false);
    let (publication, live) = super::published::published(&valid).unwrap();
    let runtime = super::manual_runtime::ManualRuntime::new();
    let (params, _) = super::constructor(&super::default_input(), &runtime, live);
    let row = ToolExecutionComponent::new(params).unwrap();
    let missing = Rc::new(
        maestro_theme::Theme::new(
            [],
            [],
            maestro_theme::ColorMode::Color256,
            maestro_theme::ThemeOptions::default(),
        )
        .unwrap(),
    );
    publication.set_theme_instance(missing).unwrap();
    row.invalidate();
    assert!(row.try_render(40).is_err());
    publication.set_theme_instance(valid).unwrap();
    row.invalidate();
    assert!(
        row.try_render(40)
            .unwrap()
            .iter()
            .any(|line| line.contains("custom_tool"))
    );
}

thread_local! {
    static DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
struct Decoded(u32);
impl<'de> serde::Deserialize<'de> for Decoded {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let value = u32::deserialize(decoder)?;
        DECODES.with(|count| count.set(count.get() + 1));
        Ok(Self(value))
    }
}
/// Each rebuild prepares fresh result data while preserving the call slot's state.
pub fn typed_rebuilds() {
    DECODES.with(|count| count.set(0));
    let runtime = super::manual_runtime::ManualRuntime::new();
    let (mut params, _) = super::params(&super::default_input(), &runtime);
    let states = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let definition = typed_definition(&states, observed.clone());
    params.tool_definition = Some(ToolRendererBinding::from_definition(&definition));
    let row = ToolExecutionComponent::new(params).unwrap();
    let result = |details| maestro_chat::ToolExecutionResult {
        content: vec![],
        details: Some(details),
        is_error: false,
    };
    row.update_result(result(serde_json::json!(7)), true)
        .unwrap();
    assert_eq!(DECODES.with(std::cell::Cell::get), 1);
    row.set_expanded(true).unwrap();
    assert_eq!(DECODES.with(std::cell::Cell::get), 2);
    row.update_result(result(serde_json::json!(9)), false)
        .unwrap();
    assert_eq!(DECODES.with(std::cell::Cell::get), 3);
    assert_eq!(
        *observed.borrow(),
        [(7, false, true, 2), (7, true, true, 3), (9, true, false, 4)]
    );
    row.update_result(result(serde_json::json!("invalid")), false)
        .unwrap();
    assert_eq!(observed.borrow().len(), 3);
    assert_eq!(*states.borrow()[0].borrow(), 5);
    assert!(
        states
            .borrow()
            .iter()
            .all(|state| Rc::ptr_eq(state, &states.borrow()[0]))
    );
    assert!(row.render(40).iter().any(|line| line.contains("call")));
}

fn typed_definition(
    states: &Rc<RefCell<Vec<Rc<RefCell<u32>>>>>,
    events: RebuildEvents,
) -> maestro_app::local::ToolDefinition<(), (), Decoded, u32> {
    use maestro_app::ToolRenderResult;
    use maestro_app::local::ToolDefinition;
    let seen = states.clone();
    ToolDefinition::<(), (), Decoded, u32> {
        definition: serde_json::from_value(
            serde_json::json!({"name":"custom","description":"","parameters":{}}),
        )
        .unwrap(),
        label: "custom".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, (), _, _, _| Box::pin(async { panic!("rendering must not execute") })),
        render_shell: Some(maestro_app::ToolRenderShell::SelfRendered),
        render_call: Some(Rc::new(move |_, _, row| {
            *row.state.borrow_mut() += 1;
            seen.borrow_mut().push(row.state.clone());
            Ok(Rc::new(maestro_tui::Text::new("call".into(), 0, 0, None)))
        })),
        render_result: Some(ToolRenderResult::<Decoded, u32>::Typed(Rc::new(
            move |result, options, _, row| {
                events.borrow_mut().push((
                    result.details.unwrap().0,
                    options.expanded,
                    options.is_partial,
                    *row.state.borrow(),
                ));
                Ok(Rc::new(maestro_tui::Text::new(
                    format!("value {}", result.details.unwrap().0),
                    0,
                    0,
                    None,
                )))
            },
        ))),
    }
}

/// Ordered result observations for each rebuild.
type RebuildEvents = Rc<RefCell<Vec<(u32, bool, bool, u32)>>>;

/// Observe the installed call and live theme through the result phase.
fn installed_call_result(
    observed: Rc<RefCell<Option<std::rc::Weak<ToolExecutionComponent>>>>,
    observed_errors: Rc<RefCell<Vec<bool>>>,
) -> ToolRenderResultCallback<Value, ToolRendererState> {
    Rc::new(move |result, _, theme, context| {
        assert!(result.content.is_empty());
        assert!(result.details.is_none());
        assert!(result.terminate.is_none());
        assert!(
            result.is_error.is_none(),
            "result payload has no error field"
        );
        observed_errors.borrow_mut().push(context.is_error);
        let row = observed.borrow().as_ref().unwrap().upgrade().unwrap();
        assert!(
            row.render(40)
                .iter()
                .any(|line| line.contains("installed call"))
        );
        let text = theme
            .get()
            .unwrap()
            .fg(&maestro_theme::ThemeColor::ToolTitle, "new theme")
            .unwrap();
        assert_eq!(text, "\x1b[38;5;61mnew theme\x1b[39m");
        Ok(Rc::new(maestro_tui::Text::new(text, 0, 0, None)))
    })
}

//! Standard custom-tool component rendering for session documents.
use crate::{
    ToolHtmlRenderer, ToolHtmlResult, ToolHtmlResultInput, ansi_lines_to_html,
    ansi_to_html::segments,
};
use maestro_models::{DiagnosticErrorInfo, UserBlock};
use maestro_theme::Theme;
use maestro_tools::{
    ToolDefinition, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase, ToolRenderResult,
    ToolRenderResultCallback, ToolRenderResultOptions, ToolRendererState,
    output::render_utils::ToolRenderResultLike,
};
use maestro_tui::tui::ComponentHandle;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, error::Error, marker::PhantomData, rc::Rc};

/// Supplied tool lookup and display context.
pub struct ToolHtmlRendererDeps<'a, F> {
    /// Live tool-definition lookup.
    pub get_tool_definition: F,
    /// Theme supplied to callbacks.
    pub theme: &'a Theme,
    /// Session working directory.
    pub cwd: &'a str,
    /// Viewport width; absence selects 100.
    pub width: Option<usize>,
    /// Existing rendering host effects.
    pub environment: Rc<ToolRenderEnvironment>,
}
/// Executable types retained without owning their values.
type ExecutableTypes<C, P, D, S> = PhantomData<fn() -> (C, P, D, S)>;

/// Retained row arguments, state and independent component slots.
struct StandardRenderer<'a, C, P, D, S, F> {
    /// Borrowed construction inputs.
    deps: ToolHtmlRendererDeps<'a, F>,
    /// Arguments retained before lookup, including absence.
    args: RefCell<HashMap<String, Rc<Value>>>,
    /// Shared mutable state per call ID.
    states: RefCell<HashMap<String, ToolRendererState>>,
    /// Previous call components.
    calls: RefCell<HashMap<String, ComponentHandle>>,
    /// Previous result components.
    results: RefCell<HashMap<String, ComponentHandle>>,
    /// Lookup's executable and details types.
    types: ExecutableTypes<C, P, D, S>,
}
/// Build the standard adapter, retaining state and components independently per call ID.
///
/// Ordinary lookup, callback, component and typed-input failures return absence.
pub fn create_tool_html_renderer<'a, C, P, D, S, F>(
    deps: ToolHtmlRendererDeps<'a, F>,
) -> impl ToolHtmlRenderer + 'a
where
    C: 'a,
    P: 'a,
    D: DeserializeOwned + 'a,
    S: Default + 'static,
    F: Fn(&str) -> Result<Option<Rc<ToolDefinition<C, P, D, S>>>, DiagnosticErrorInfo> + 'a,
{
    StandardRenderer {
        deps,
        args: RefCell::default(),
        states: RefCell::default(),
        calls: RefCell::default(),
        results: RefCell::default(),
        types: PhantomData,
    }
}
impl<C, P, D, S: Default + 'static, F> StandardRenderer<'_, C, P, D, S, F> {
    /// Retain the row owner in the invoking frame, outside the ID table's borrow.
    fn state(&self, id: &str) -> ToolRendererState {
        let existing = self.states.borrow().get(id).cloned();
        existing.unwrap_or_else(|| {
            let state = ToolRendererState::default();
            self.states
                .borrow_mut()
                .insert(id.to_owned(), state.clone());
            state
        })
    }
    /// Construct a context without holding row borrows across caller code.
    fn context<'a>(
        &self,
        id: &str,
        args: Option<&'a Value>,
        owner: &'a ToolRendererState,
        flags: (Option<bool>, bool),
    ) -> ToolRenderContext<'a, S> {
        let (result, expanded) = flags;
        let state = owner.state::<S>();
        let slots = if result.is_some() {
            &self.results
        } else {
            &self.calls
        };
        let last_component = slots.borrow().get(id).cloned();
        ToolRenderContext {
            args,
            tool_call_id: id.to_owned(),
            invalidate: Rc::new(|| {}),
            environment: Rc::clone(&self.deps.environment),
            last_component,
            state,
            renderer_state: Some(owner),
            cwd: self.deps.cwd.to_owned(),
            phase: ToolRenderPhase {
                execution_started: true,
                args_complete: true,
                is_partial: result.is_none(),
            },
            expanded,
            show_images: false,
            is_error: result.unwrap_or(false),
        }
    }
    /// Publish a component before its fallible render; drop the replaced handle outside the borrow.
    fn render(
        &self,
        id: &str,
        component: &ComponentHandle,
        is_result: bool,
    ) -> Result<String, Box<dyn Error>> {
        let slots = if is_result {
            &self.results
        } else {
            &self.calls
        };
        let previous = slots
            .borrow_mut()
            .insert(id.to_owned(), Rc::clone(component));
        drop(previous);
        let lines = component.try_render(self.deps.width.unwrap_or(100))?;
        Ok(ansi_lines_to_html(if is_result {
            trim_result_lines(&lines)
        } else {
            &lines
        }))
    }
    /// Invoke and render one result phase, observing live retained arguments and slots.
    fn result_phase<T>(
        &self,
        id: &str,
        callback: &ToolRenderResultCallback<T, S>,
        result: &ToolRenderResultLike<'_, T>,
        expanded: bool,
    ) -> Option<String> {
        let retained = self.args.borrow().get(id).cloned();
        let owner = self.state(id);
        let row = self.context(id, retained.as_deref(), &owner, (result.is_error, expanded));
        let component = callback(
            result,
            ToolRenderResultOptions {
                expanded,
                is_partial: false,
            },
            self.deps.theme,
            &row,
        )
        .ok()?;
        self.render(id, &component, true).ok()
    }
    /// Render both phases with one borrowed details instance.
    fn result_phases<T>(
        &self,
        id: &str,
        callback: &ToolRenderResultCallback<T, S>,
        result: &ToolRenderResultLike<'_, T>,
    ) -> Option<ToolHtmlResult> {
        let collapsed = self.result_phase(id, callback, result, false)?;
        let expanded = self.result_phase(id, callback, result, true)?;
        Some(ToolHtmlResult {
            collapsed: (!collapsed.is_empty() && collapsed != expanded).then_some(collapsed),
            expanded: Some(expanded),
        })
    }
}
impl<C, P, D, S, F> ToolHtmlRenderer for StandardRenderer<'_, C, P, D, S, F>
where
    D: DeserializeOwned,
    S: Default + 'static,
    F: Fn(&str) -> Result<Option<Rc<ToolDefinition<C, P, D, S>>>, DiagnosticErrorInfo>,
{
    fn render_call(
        &self,
        id: &str,
        name: &str,
        args: Option<&Value>,
    ) -> Result<Option<String>, Box<dyn Error>> {
        let retained = args.map(|args| Rc::new(args.clone()));
        let previous = {
            let mut stored = self.args.borrow_mut();
            if let Some(args) = retained {
                stored.insert(id.to_owned(), args)
            } else {
                stored.remove(id)
            }
        };
        drop(previous);
        let output = (|| {
            let tool = (self.deps.get_tool_definition)(name).ok()??;
            let callback = tool.render_call.as_ref()?;
            let retained = self.args.borrow().get(id).cloned();
            let owner = self.state(id);
            let context = self.context(id, retained.as_deref(), &owner, (None, false));
            let component = callback(args, self.deps.theme, &context).ok()?;
            self.render(id, &component, false).ok()
        })();
        Ok(output)
    }
    fn render_result(
        &self,
        id: &str,
        name: &str,
        input: ToolHtmlResultInput<'_>,
    ) -> Result<Option<ToolHtmlResult>, Box<dyn Error>> {
        let output = (|| {
            let tool = (self.deps.get_tool_definition)(name).ok()??;
            let callback = tool.render_result.as_ref()?;
            let content: Vec<UserBlock> = input
                .content
                .map(Vec::deserialize)
                .transpose()
                .ok()?
                .unwrap_or_default();
            match callback {
                ToolRenderResult::Typed(callback) => {
                    let details = input.details.map(D::deserialize).transpose().ok()?;
                    self.result_phases(
                        id,
                        callback,
                        &ToolRenderResultLike {
                            content: &content,
                            details: details.as_ref(),
                            terminate: None,
                            is_error: Some(input.is_error),
                        },
                    )
                }
                ToolRenderResult::Raw(callback) => self.result_phases(
                    id,
                    callback,
                    &ToolRenderResultLike {
                        content: &content,
                        details: input.details,
                        terminate: None,
                        is_error: Some(input.is_error),
                    },
                ),
            }
        })();
        Ok(output)
    }
}
/// Whether a line has no non-whitespace text after recognized SGR removal.
fn is_blank(line: &str) -> bool {
    let mut blank = true;
    segments(line, |plain, _| {
        blank &= plain.chars().all(maestro_text::is_whitespace_scalar);
    });
    blank
}
/// Borrow only the nonblank result range, leaving interior lines unchanged.
fn trim_result_lines(lines: &[String]) -> &[String] {
    let start = lines
        .iter()
        .position(|line| !is_blank(line))
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !is_blank(line))
        .map_or(start, |last| last + 1);
    &lines[start..end]
}

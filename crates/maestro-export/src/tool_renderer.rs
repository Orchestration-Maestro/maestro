//! Standard custom-tool component rendering for session documents.
use crate::{
    ToolHtmlRenderer, ToolHtmlResult, ToolHtmlResultInput, ansi_lines_to_html,
    ansi_to_html::segments,
};
use maestro_models::{DiagnosticErrorInfo, UserBlock};
use maestro_theme::Theme;
use maestro_tools::{
    ToolDefinition, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase, ToolRenderResult,
    ToolRenderResultOptions, output::render_utils::ToolRenderResultLike,
};
use maestro_tui::tui::ComponentHandle;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, error::Error, marker::PhantomData, rc::Rc};

/// Supplied tool lookup, display context and typed empty-state construction.
pub struct ToolHtmlRendererDeps<'a, F, S> {
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
    /// Construct the empty state when a callback first needs it.
    pub create_state: fn() -> S,
}
/// Executable types retained without owning their values.
type ExecutableTypes<C, P, D> = PhantomData<fn() -> (C, P, D)>;

/// Retained row arguments, state and independent component slots.
struct StandardRenderer<'a, C, P, D, S, F> {
    /// Borrowed construction inputs.
    deps: ToolHtmlRendererDeps<'a, F, S>,
    /// Arguments retained before lookup, including absence.
    args: RefCell<HashMap<String, Rc<Value>>>,
    /// Shared mutable state per call ID.
    states: RefCell<HashMap<String, Rc<RefCell<S>>>>,
    /// Previous call components.
    calls: RefCell<HashMap<String, ComponentHandle>>,
    /// Previous result components.
    results: RefCell<HashMap<String, ComponentHandle>>,
    /// Lookup's executable and details types.
    types: ExecutableTypes<C, P, D>,
}
/// Build the standard adapter, retaining state and components independently per call ID.
///
/// Ordinary lookup, callback, component and typed-input failures return absence.
pub fn create_tool_html_renderer<'a, C, P, D, S, F>(
    deps: ToolHtmlRendererDeps<'a, F, S>,
) -> impl ToolHtmlRenderer + 'a
where
    C: 'a,
    P: 'a,
    D: DeserializeOwned + 'a,
    S: 'a,
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
impl<C, P, D, S, F> StandardRenderer<'_, C, P, D, S, F> {
    /// Construct a context without holding row borrows across caller code.
    fn context<'a>(
        &self,
        id: &str,
        args: Option<&'a Value>,
        result: Option<bool>,
        expanded: bool,
    ) -> ToolRenderContext<'a, S> {
        let existing = self.states.borrow().get(id).cloned();
        let state = existing.unwrap_or_else(|| {
            let state = Rc::new(RefCell::new((self.deps.create_state)()));
            let previous = self
                .states
                .borrow_mut()
                .insert(id.to_owned(), Rc::clone(&state));
            drop(previous);
            state
        });
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
    fn result_phase(
        &self,
        id: &str,
        callback: &ToolRenderResult<D, S>,
        result: &ToolRenderResultLike<'_, D>,
        expanded: bool,
    ) -> Option<String> {
        let retained = self.args.borrow().get(id).cloned();
        let row = self.context(id, retained.as_deref(), result.is_error, expanded);
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
}
impl<C, P, D, S, F> ToolHtmlRenderer for StandardRenderer<'_, C, P, D, S, F>
where
    D: DeserializeOwned,
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
            let context = self.context(id, retained.as_deref(), None, false);
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
            let details = input.details.map(D::deserialize).transpose().ok()?;
            let result = ToolRenderResultLike {
                content: &content,
                details: details.as_ref(),
                terminate: None,
                is_error: Some(input.is_error),
            };
            let collapsed = self.result_phase(id, callback, &result, false)?;
            let expanded = self.result_phase(id, callback, &result, true)?;
            Some(ToolHtmlResult {
                collapsed: (!collapsed.is_empty() && collapsed != expanded).then_some(collapsed),
                expanded: Some(expanded),
            })
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

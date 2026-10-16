//! Standard custom-tool component rendering for session documents.
use crate::{
    ToolHtmlRenderer, ToolHtmlResult, ToolHtmlResultInput, ansi_lines_to_html,
    ansi_to_html::segments,
};
use maestro_models::{DiagnosticErrorInfo, UserBlock};
use maestro_theme::Theme;
use maestro_tools::{
    PreparedToolRenderResult, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase,
    ToolRenderResultOptions, ToolRendererBinding, ToolRendererState,
    output::render_utils::ToolRenderResultLike,
};
use maestro_tui::tui::ComponentHandle;
use serde::Deserialize;
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, error::Error, rc::Rc};

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
/// Retained row arguments, state and independent component slots.
struct StandardRenderer<'a, F> {
    /// Borrowed construction inputs.
    deps: ToolHtmlRendererDeps<'a, F>,
    /// Arguments retained before lookup, including absence.
    args: RefCell<HashMap<String, Rc<Value>>>,
    /// Shared mutable state per call ID.
    states: RefCell<HashMap<String, Rc<RefCell<ToolRendererState>>>>,
    /// Previous call components.
    calls: RefCell<HashMap<String, ComponentHandle>>,
    /// Previous result components.
    results: RefCell<HashMap<String, ComponentHandle>>,
}
/// Build the standard adapter, retaining state and components independently per call ID.
///
/// Ordinary lookup, callback, component and typed-input failures return absence.
pub fn create_tool_html_renderer<'a, F>(
    deps: ToolHtmlRendererDeps<'a, F>,
) -> impl ToolHtmlRenderer + 'a
where
    F: Fn(&str) -> Result<Option<ToolRendererBinding>, DiagnosticErrorInfo> + 'a,
{
    StandardRenderer {
        deps,
        args: RefCell::default(),
        states: RefCell::default(),
        calls: RefCell::default(),
        results: RefCell::default(),
    }
}
impl<F> StandardRenderer<'_, F> {
    /// Retain the row owner in the invoking frame, outside the ID table's borrow.
    fn state(&self, id: &str) -> Rc<RefCell<ToolRendererState>> {
        let existing = self.states.borrow().get(id).cloned();
        existing.unwrap_or_else(|| {
            let state = Rc::new(RefCell::new(ToolRendererState::default()));
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
        owners: (&Rc<RefCell<ToolRendererState>>, &'a ToolRendererState),
        flags: (Option<bool>, bool),
    ) -> ToolRenderContext<'a, ToolRendererState> {
        let (result, expanded) = flags;
        let (state, owner) = owners;
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
            state: Rc::clone(state),
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
    fn result_phase(
        &self,
        id: &str,
        prepared: &PreparedToolRenderResult<'_>,
        flags: (bool, bool),
    ) -> Option<String> {
        let (is_error, expanded) = flags;
        let retained = self.args.borrow().get(id).cloned();
        let owner = self.state(id);
        let inner = owner.borrow().clone();
        let row = self.context(
            id,
            retained.as_deref(),
            (&owner, &inner),
            (Some(is_error), expanded),
        );
        let component = prepared
            .render(
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
    fn result_phases(
        &self,
        id: &str,
        prepared: &PreparedToolRenderResult<'_>,
        is_error: bool,
    ) -> Option<ToolHtmlResult> {
        let collapsed = self.result_phase(id, prepared, (is_error, false))?;
        let expanded = self.result_phase(id, prepared, (is_error, true))?;
        Some(ToolHtmlResult {
            collapsed: (!collapsed.is_empty() && collapsed != expanded).then_some(collapsed),
            expanded: Some(expanded),
        })
    }
}
impl<F> ToolHtmlRenderer for StandardRenderer<'_, F>
where
    F: Fn(&str) -> Result<Option<ToolRendererBinding>, DiagnosticErrorInfo>,
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
            let inner = owner.borrow().clone();
            let context = self.context(id, retained.as_deref(), (&owner, &inner), (None, false));
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
            let prepared = callback
                .prepare(ToolRenderResultLike {
                    content: &content,
                    details: input.details,
                    terminate: None,
                    is_error: Some(input.is_error),
                })
                .ok()?;
            self.result_phases(id, &prepared, input.is_error)
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

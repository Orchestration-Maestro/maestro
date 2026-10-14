//! Retained edit previews on the caller's terminal runtime.
use crate::output::render_utils::{ToolRenderResultLike, render_args};

use crate::{
    diff::{RenderDiffOptions, render_diff},
    edit_diff::{Edit, EditDiffError, EditDiffResult, compute_edits_diff},
    output::{
        render_parts::{downcast, float, paint, themed},
        render_utils::{number, path_display, selected_path},
    },
    types::{ToolRenderContext, ToolRenderResultOptions},
    utils::diagnostics::diagnostic,
};
use maestro_models::{DiagnosticErrorInfo, UserBlock};
use maestro_theme::{Theme, ThemeBg, ThemeColor};
use maestro_tui::{Box as TuiBox, Component, Container, Spacer, Text, tui::ComponentHandle};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

/// Fallible rendering output.
type Rendered<T> = Result<T, DiagnosticErrorInfo>;
/// A read-only preview or its caught failure.
type EditPreview = Result<PreviewDetails, EditDiffError>;
/// Display metadata retains numeric identity independently of native execution indices.
#[derive(PartialEq)]
struct PreviewDetails {
    /// Numbered diff text.
    diff: String,
    /// Optional source-provided line identity.
    first_changed_line: Option<f64>,
}

/// Retained call component shared by the edit definition's renderers.
#[derive(Default)]
pub struct EditRenderState {
    /// The call slot, distinct from the result slot.
    call_component: Option<Rc<EditCallRenderComponent>>,
}

/// Selected authored input, preserving replacement order and duplicates.
#[derive(PartialEq, Eq)]
struct PreviewInput {
    /// Authored preview path, independent of the title's alias preference.
    path: String,
    /// Original-snapshot replacements.
    edits: Vec<Edit>,
}

/// Mutable preview state, separate from the widget's children.
#[derive(Default)]
struct PreviewState {
    /// Current selected input.
    input: Option<PreviewInput>,
    /// Published preview.
    preview: Option<EditPreview>,
    /// Authority of the currently outstanding request.
    request: Option<Rc<()>>,
    /// Whether a result has revoked preview admission for these arguments.
    settled: bool,
    /// Execution error flag, subordinate to an existing preview.
    settled_error: bool,
}

/// Private edit component forwarding layout to the toolkit box.
struct EditCallRenderComponent {
    /// Existing padded layout.
    frame: TuiBox,
    /// Read-only preview publication and admission.
    state: RefCell<PreviewState>,
}

impl Component for EditCallRenderComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.frame.render(width)
    }
    fn invalidate(&self) {
        self.frame.invalidate();
    }
}

/// Prefer a previous edit component, then row state, then an empty box.
fn call_component(context: &ToolRenderContext<'_, EditRenderState>) -> Rc<EditCallRenderComponent> {
    let previous = context.last_component.as_ref().and_then(downcast);
    let retained = context.state.borrow().call_component.clone();
    let component = previous.or(retained).unwrap_or_else(|| {
        Rc::new(EditCallRenderComponent {
            frame: TuiBox::new(1, 1, None),
            state: RefCell::new(PreviewState::default()),
        })
    });
    let replaced = context
        .state
        .borrow_mut()
        .call_component
        .replace(Rc::clone(&component));
    drop(replaced);
    component
}

/// Select complete preview input without applying execution preparation.
fn preview_input(args: &Value) -> Option<PreviewInput> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .or_else(|| args.get("file_path").and_then(Value::as_str))?;
    if path.is_empty() {
        return None;
    }
    let array = args
        .get("edits")
        .and_then(Value::as_array)
        .filter(|edits| !edits.is_empty());
    let edits = array
        .and_then(|edits| {
            edits
                .iter()
                .map(|edit| {
                    Some(Edit {
                        old_text: edit.get("oldText")?.as_str()?.to_owned(),
                        new_text: edit.get("newText")?.as_str()?.to_owned(),
                    })
                })
                .collect::<Option<Vec<_>>>()
        })
        .or_else(|| {
            Some(vec![Edit {
                old_text: args.get("oldText")?.as_str()?.to_owned(),
                new_text: args.get("newText")?.as_str()?.to_owned(),
            }])
        })?;
    Some(PreviewInput {
        path: path.to_owned(),
        edits,
    })
}

/// Format the title using the display path's independent alias precedence.
fn format_call(args: &Value, theme: &Theme) -> Rendered<String> {
    let path = path_display(selected_path(args), theme)?;
    let title = paint(theme, &ThemeColor::ToolTitle, &Theme::bold("edit", true))?;
    Ok(format!("{title} {path}"))
}

/// Preview state takes precedence over execution status for the header.
fn header_color(state: &PreviewState) -> ThemeBg {
    match state.preview {
        Some(Err(_)) => ThemeBg::ToolErrorBg,
        Some(Ok(_)) => ThemeBg::ToolSuccessBg,
        None if state.settled_error => ThemeBg::ToolErrorBg,
        None => ThemeBg::ToolPendingBg,
    }
}

/// Install a preview and revoke its outstanding request even when text is unchanged.
fn set_preview(state: &mut PreviewState, preview: EditPreview) -> bool {
    let changed = state.preview.as_ref() != Some(&preview);
    state.preview = Some(preview);
    state.request = None;
    changed
}

/// Rebuild title and body using existing widgets; the background samples the live theme.
fn build_call(
    component: &Rc<EditCallRenderComponent>,
    args: &Value,
    theme: &Theme,
    context: &ToolRenderContext<'_, EditRenderState>,
) -> Rendered<()> {
    let color = header_color(&component.state.borrow());
    themed(themed(context.environment.live_theme.get())?.bg(&color, ""))?;
    let weak = Rc::downgrade(component);
    let live = context.environment.live_theme.clone();
    component.frame.set_bg_fn(Some(Rc::new(move |text| {
        let color = weak
            .upgrade()
            .map(|component| header_color(&component.state.borrow()));
        color
            .and_then(|color| live.get().ok()?.bg(&color, text).ok())
            .unwrap_or_else(|| text.to_owned())
    })));
    component.frame.clear();
    component
        .frame
        .add_child(Rc::new(Text::new(format_call(args, theme)?, 0, 0, None)));
    let body = match component.state.borrow().preview.as_ref() {
        None => None,
        Some(Err(error)) => Some(paint(theme, &ThemeColor::Error, &error.error)?),
        Some(Ok(preview)) => Some(themed(render_diff(
            &preview.diff,
            &RenderDiffOptions::default(),
            &context.environment.live_theme,
        ))?),
    };
    if let Some(body) = body {
        component.frame.add_child(Rc::new(Spacer::new(1)));
        component
            .frame
            .add_child(Rc::new(Text::new(body, 0, 0, None)));
    }
    Ok(())
}

/// Admit one preview before formatting, publishing only while its request is current.
fn admit_preview(
    component: &Rc<EditCallRenderComponent>,
    input: Option<PreviewInput>,
    context: &ToolRenderContext<'_, EditRenderState>,
) -> Rendered<()> {
    let work = {
        let mut state = component.state.borrow_mut();
        if state.input != input {
            *state = PreviewState {
                input,
                ..PreviewState::default()
            };
        }
        if !context.phase.args_complete
            || state.preview.is_some()
            || state.request.is_some()
            || state.settled
        {
            return Ok(());
        }
        let Some(input) = state.input.as_ref() else {
            return Ok(());
        };
        let work = compute_edits_diff(&input.path, input.edits.clone(), &context.cwd)
            .map_err(|error| diagnostic(&error))?;
        let request = Rc::new(());
        state.request = Some(Rc::clone(&request));
        (work, request)
    };
    let weak = Rc::downgrade(component);
    let invalidate = Rc::clone(&context.invalidate);
    context
        .environment
        .runtime
        .spawn_local(Box::pin(async move {
            let preview = work.0.await;
            if let Some(component) = weak.upgrade() {
                let accepted = publish(&component, &work.1, preview);
                if accepted {
                    invalidate();
                }
            }
            Ok(())
        }));
    Ok(())
}

/// Publish only to the outstanding request, releasing state before invalidation.
fn publish(
    component: &EditCallRenderComponent,
    request: &Rc<()>,
    preview: Result<EditDiffResult, EditDiffError>,
) -> bool {
    let mut state = component.state.borrow_mut();
    if !state
        .request
        .as_ref()
        .is_some_and(|current| Rc::ptr_eq(current, request))
    {
        return false;
    }
    set_preview(
        &mut state,
        preview.map(|preview| PreviewDetails {
            diff: preview.diff,
            first_changed_line: preview.first_changed_line.map(float),
        }),
    );
    true
}

/// Render the call after admitting complete input on the supplied runtime.
pub(super) fn render_call(
    args: Option<&Value>,
    theme: &Theme,
    context: &ToolRenderContext<'_, EditRenderState>,
) -> Rendered<ComponentHandle> {
    let args = render_args(args);
    let component = call_component(context);
    admit_preview(&component, preview_input(args), context)?;
    build_call(&component, args, theme, context)?;
    Ok(component)
}

/// Emit distinct result text only; ordinary successful execution text is hidden.
fn format_result(
    preview: Option<&EditPreview>,
    result: &ToolRenderResultLike<'_, Value>,
    theme: &Theme,
    context: &ToolRenderContext<'_, EditRenderState>,
) -> Rendered<Option<String>> {
    if context.is_error {
        let text = result
            .content
            .iter()
            .filter_map(|block| match block {
                UserBlock::Text(text) => Some(text.text.as_str()),
                UserBlock::Image(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty()
            || preview.is_some_and(|preview| {
                preview
                    .as_ref()
                    .err()
                    .is_some_and(|error| error.error == text)
            })
        {
            return Ok(None);
        }
        return paint(theme, &ThemeColor::Error, &text).map(Some);
    }
    let Some(details) = result
        .details
        .and_then(|details| details.get("diff"))
        .and_then(Value::as_str)
        .filter(|diff| !diff.is_empty())
    else {
        return Ok(None);
    };
    if preview.is_some_and(|preview| {
        preview
            .as_ref()
            .is_ok_and(|preview| preview.diff == details)
    }) {
        return Ok(None);
    }
    themed(render_diff(
        details,
        &RenderDiffOptions::default(),
        &context.environment.live_theme,
    ))
    .map(Some)
}

/// Revoke pending work and install successful result details on an existing call.
fn settle(
    component: &Rc<EditCallRenderComponent>,
    result: &ToolRenderResultLike<'_, Value>,
    context: &ToolRenderContext<'_, EditRenderState>,
) -> bool {
    let input = preview_input(render_args(context.args));
    let mut state = component.state.borrow_mut();
    state.request = None;
    state.settled = true;
    let mut changed = false;
    if !context.is_error
        && let Some(diff) = result
            .details
            .and_then(|details| details.get("diff"))
            .and_then(Value::as_str)
    {
        changed = set_preview(
            &mut state,
            Ok(PreviewDetails {
                diff: diff.to_owned(),
                first_changed_line: number(result.details, "firstChangedLine"),
            }),
        );
        state.input = input;
    }
    changed |= state.settled_error != context.is_error;
    state.settled_error = context.is_error;
    changed
}

/// Render a result, reusing the detached result slot and rebuilding a retained call if changed.
pub(super) fn render_result(
    result: &ToolRenderResultLike<'_, Value>,
    _options: ToolRenderResultOptions,
    theme: &Theme,
    context: &ToolRenderContext<'_, EditRenderState>,
) -> Rendered<ComponentHandle> {
    let call = context.state.borrow().call_component.clone();
    if let Some(call) = &call
        && settle(call, result, context)
    {
        build_call(call, render_args(context.args), theme, context)?;
    }
    let output = match &call {
        Some(call) => format_result(call.state.borrow().preview.as_ref(), result, theme, context)?,
        None => format_result(None, result, theme, context)?,
    };
    let component = context
        .last_component
        .as_ref()
        .and_then(downcast::<Container>)
        .unwrap_or_else(|| Rc::new(Container::new()));
    component.clear();
    if let Some(output) = output {
        component.add_child(Rc::new(Spacer::new(1)));
        component.add_child(Rc::new(Text::new(output, 1, 0, None)));
    }
    Ok(component)
}

#[cfg(test)]
mod tests;

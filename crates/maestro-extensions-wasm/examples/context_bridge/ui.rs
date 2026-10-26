//! Canonical retained UI methods use the host's original capability.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortSignal, CallbackEmitter, Component, ComponentDisposer, ComponentFactory, ComponentView,
    ExtensionAPI, ExtensionComponent, ExtensionContext, ExtensionResult, ExtensionUIDialogOptions,
    ExtensionWidgetOptions, FlagOptions, FlagType, FooterData, FooterFactory, NotificationType,
    Subscription, TerminalInputResult, ThemeSelection, ThemeSetResult, WidgetPlacement,
    WorkingIndicatorOptions,
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Supplies distinguishable operands, then separately exercises host defaults and clearing.
pub(super) fn observe(
    context: &ExtensionContext,
    api: &ExtensionAPI,
    args: &str,
) -> ExtensionResult<String> {
    if let Some(operation) = args.strip_prefix("editor ") {
        return editor(context, operation);
    }
    if let Some(request) = args.strip_prefix("ui terminal ") {
        return terminal(context, request);
    }
    if let Some(request) = args.strip_prefix("ui theme ") {
        return themes(context, request);
    }
    if let Some(steps) = args.strip_prefix("ui widgets ") {
        let steps = serde_json::from_str(steps).map_err(|error| error.to_string())?;
        return Ok(super::widget_author::run(steps, context.ui().ok()).to_string());
    }
    if let Some(name) = args.strip_prefix("ui provider ") {
        super::stacked_provider_author::install(&context.ui()?, name)?;
        return Ok("provider stacked".into());
    }
    if let Some(kind) = args.strip_prefix("ui editor ") {
        return Ok(super::editor_factory_author::install(&context.ui()?, kind)?.to_string());
    }
    if let Some(options) = args.strip_prefix("ui capabilities ") {
        let (width, expandable): (Option<f64>, bool) =
            serde_json::from_str(options).map_err(|e| e.to_string())?;
        super::factory_component_author::install(&context.ui()?, width, expandable)?;
        return Ok("capabilities shown".into());
    }
    if let Some(label) = args.strip_prefix("ui widget ") {
        return widget(context, api, label);
    }
    let clear = args == "ui clear";
    let ui = context.ui()?;
    ui.notify(
        " literal notification ",
        if clear {
            None
        } else {
            Some(NotificationType::Warning)
        },
    )?;
    ui.set_status(" literal key ", (!clear).then_some(""))?;
    ui.set_working_message((!clear).then_some(" literal working "))?;
    ui.set_working_visible(!clear)?;
    ui.set_working_indicator((!clear).then(|| WorkingIndicatorOptions {
        frames: Some(vec![]),
        interval_ms: Some(2.5),
    }))?;
    ui.set_hidden_thinking_label((!clear).then_some(" literal hidden "))?;
    let lines = vec![" literal line ".into(), String::new()];
    ui.set_widget(
        " literal widget ",
        (!clear).then_some(lines.as_slice()),
        (!clear).then_some(ExtensionWidgetOptions {
            placement: Some(WidgetPlacement::BelowEditor),
        }),
    )?;
    ui.set_title(if clear { "" } else { " literal title " })?;
    Ok("UI forwarded".into())
}

/// Shows the dialog `request` names on the originally acquired UI, with an already aborted
/// caller signal and the timeout it asks for, and returns its answer.
pub(super) async fn dialog(context: &ExtensionContext, request: &str) -> ExtensionResult<String> {
    let request: Value = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let text = |key: &str| request[key].as_str().map(str::to_owned);
    let title = text("title").unwrap_or_default();
    let signal = request["aborted"].as_bool();
    let timeout = request["timeout"].as_f64();
    let opts = (signal.is_some() || timeout.is_some()).then(|| ExtensionUIDialogOptions {
        signal: signal.map(|_| AbortSignal::new(Rc::new(super::exec::AlreadyAborted))),
        timeout,
    });
    let ui = context.ui()?;
    let answer = match request["method"].as_str() {
        Some("select") => {
            let options = serde_json::from_value(request["options"].clone());
            let options = options.map_err(|error| error.to_string())?;
            json!(ui.select(title, options, opts).await?)
        }
        Some("confirm") => {
            let message = text("message").unwrap_or_default();
            json!(ui.confirm(title, message, opts).await?)
        }
        Some("input") => json!(ui.input(title, text("placeholder"), opts).await?),
        _ => json!(ui.editor(title, text("prefill")).await?),
    };
    Ok(answer.to_string())
}

/// A factory widget that renders its label and width, and keeps a capture released with it.
struct Labelled {
    /// The label it renders.
    label: String,
    /// Released with the component.
    _capture: super::signals::Released,
}

impl ComponentView for Labelled {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![format!("{} {width}", self.label)])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A factory whose component renders `label` and its width, whose disposal registers the flag
/// `disposed-<label>` and whose capture registers `released-<label>` once it is released.
fn labelled(api: &ExtensionAPI, label: &str) -> ComponentFactory {
    let (api, label) = (api.clone(), label.to_owned());
    Rc::new(move |_, _, _| {
        let (owner, named) = (api.clone(), label.clone());
        let dispose: ComponentDisposer = Rc::new(move |_| {
            let flag = FlagOptions {
                description: None,
                type_: FlagType::Boolean,
                default: None,
            };
            owner.register_flag(&format!("disposed-{named}"), flag)
        });
        let capture = super::signals::Released(api.clone(), label.clone());
        Ok(ExtensionComponent {
            component: Component::new(Labelled {
                label: label.clone(),
                _capture: capture,
            }),
            dispose: Some(dispose),
            set_expanded: None,
            width: None,
        })
    })
}

/// A footer that renders its label and the footer metadata it reads on every render.
struct FooterView {
    /// The label it renders.
    label: String,
    /// The footer metadata its factory received.
    footer: FooterData,
}

impl ComponentView for FooterView {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let branch = self.footer.get_git_branch()?;
        let statuses = self.footer.get_extension_statuses()?;
        let providers = self.footer.get_available_provider_count()?;
        Ok(vec![
            json!([self.label, branch, statuses, providers]).to_string(),
        ])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A footer factory whose branch listener registers the flag `branch-<label>`.
fn footer(api: &ExtensionAPI, label: &str) -> FooterFactory {
    let (api, label) = (api.clone(), label.to_owned());
    Rc::new(move |_, _, footer, _| {
        let (owner, named) = (api.clone(), label.clone());
        let subscription = footer.on_branch_change(Rc::new(move || {
            let flag = FlagOptions {
                description: None,
                type_: FlagType::Boolean,
                default: None,
            };
            owner.register_flag(&format!("branch-{named}"), flag)
        }))?;
        drop(subscription);
        let view = FooterView {
            label: label.clone(),
            footer,
        };
        Ok(ExtensionComponent {
            component: Component::new(view),
            dispose: None,
            set_expanded: None,
            width: None,
        })
    })
}

/// Shows a labelled factory widget, or replaces the header with one; `header` alone restores
/// the host's header.
fn widget(
    context: &ExtensionContext,
    api: &ExtensionAPI,
    request: &str,
) -> ExtensionResult<String> {
    let ui = context.ui()?;
    match request.split_once(' ') {
        Some(("header", label)) => ui.set_header(Some(labelled(api, label)))?,
        Some(("footer", label)) => ui.set_footer(Some(footer(api, label)))?,
        None if request == "footer" => ui.set_footer(None)?,
        None if request == "header" => ui.set_header(None)?,
        _ => ui.set_widget_factory("bridge", labelled(api, request), None)?,
    }
    Ok("widget shown".into())
}

/// Runs the theme or tool expansion request `request` names and returns its answer as JSON.
fn themes(context: &ExtensionContext, request: &str) -> ExtensionResult<String> {
    let ui = context.ui()?;
    let (action, operand) = request.split_once(' ').unwrap_or((request, ""));
    let selected = |result: ThemeSetResult| json!([result.success, result.error]);
    let answer = match action {
        "current" => json!(ui.theme()?.name()?),
        "all" => json!(
            ui.get_all_themes()?
                .into_iter()
                .map(|it| (it.name, it.path))
                .collect::<Vec<_>>()
        ),
        "load" => json!(ui.get_theme(operand)?.map(|it| it.name()).transpose()?),
        "select" => selected(ui.set_theme(ThemeSelection::Name(operand.to_owned()))?),
        "select-loaded" => {
            let theme = ui.get_theme(operand)?.ok_or("no theme to select")?;
            selected(ui.set_theme(ThemeSelection::Theme(theme))?)
        }
        "expanded" => json!(ui.get_tools_expanded()?),
        _ => json!(ui.set_tools_expanded(operand == "true")?),
    };
    Ok(answer.to_string())
}

thread_local! {
    /// Terminal input subscriptions by label.
    static LISTENING: RefCell<HashMap<String, Subscription>> = RefCell::default();
}

/// Adds, unsubscribes or drops the labelled terminal input listener `request` names. The
/// listener consumes `consume`, passes `pass` on, fails on `fail` and otherwise replaces the
/// chunk with its label and the chunk.
fn terminal(context: &ExtensionContext, request: &str) -> ExtensionResult<String> {
    let (action, label) = request.split_once(' ').ok_or("missing listener label")?;
    if action != "listen" {
        let subscription = LISTENING.with_borrow_mut(|listening| listening.remove(label));
        let subscription = subscription.ok_or("unknown listener")?;
        if action == "unsubscribe" {
            subscription.unsubscribe()?;
        }
        return Ok(format!("{action} {label}"));
    }
    let named = label.to_owned();
    let subscription = context.ui()?.on_terminal_input(Rc::new(move |data, _| {
        let decision = |consume, data| Some(TerminalInputResult { consume, data });
        match data {
            "consume" => Ok(decision(Some(true), Some("ignored".into()))),
            "pass" => Ok(None),
            "fail" => Err(format!("{named} failed")),
            _ => Ok(decision(None, Some(format!("{named}:{data}")))),
        }
    }))?;
    LISTENING.with_borrow_mut(|listening| listening.insert(label.to_owned(), subscription));
    Ok(format!("listen {label}"))
}

/// Reads and edits the originally acquired UI without interpreting the supplied text.
fn editor(context: &ExtensionContext, operation: &str) -> ExtensionResult<String> {
    let ui = context.ui()?;
    if let Some(text) = operation.strip_prefix("paste:") {
        ui.paste_to_editor(text)?;
    } else if let Some(text) = operation.strip_prefix("set:") {
        ui.set_editor_text(text)?;
    } else {
        return ui.get_editor_text();
    }
    Ok("editor forwarded".into())
}

//! Canonical retained UI methods use the host's original capability.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortSignal, ExtensionContext, ExtensionResult, ExtensionUIDialogOptions,
    ExtensionWidgetOptions, NotificationType, Subscription, TerminalInputResult, ThemeSelection,
    ThemeSetResult, WidgetPlacement, WorkingIndicatorOptions,
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Supplies distinguishable operands, then separately exercises host defaults and clearing.
pub(super) fn observe(context: &ExtensionContext, args: &str) -> ExtensionResult<String> {
    if let Some(operation) = args.strip_prefix("editor ") {
        return editor(context, operation);
    }
    if let Some(request) = args.strip_prefix("ui terminal ") {
        return terminal(context, request);
    }
    if let Some(request) = args.strip_prefix("ui theme ") {
        return themes(context, request);
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

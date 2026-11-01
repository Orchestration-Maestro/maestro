//! Author operations on themes retained from renderer invocations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    ColorMode, CommandOptions, ExtensionAPI, ExtensionResult, Theme, ThemeBg, ThemeColor,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// One method call on a retained theme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    /// Retention order of the theme.
    index: usize,
    /// Theme method, in its reference spelling.
    method: String,
    /// Positional operands.
    args: Vec<Value>,
}

/// The text operand at `index`.
fn text(args: &[Value], index: usize) -> ExtensionResult<&str> {
    args.get(index)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing text operand {index}"))
}

/// Runs one theme method and returns its answer.
fn perform(theme: &Theme, method: &str, args: &[Value]) -> ExtensionResult<Value> {
    Ok(match method {
        "name" => json!(theme.name()?),
        "sourcePath" => json!(theme.source_path()?),
        "sourceInfo" => json!(theme.source_info()?),
        "setSourceInfo" => {
            let source = serde_json::from_value(args.first().cloned().unwrap_or_default())
                .map_err(|error| error.to_string())?;
            theme.set_source_info(source)?;
            Value::Null
        }
        "fg" => {
            let color: &ThemeColor = text(args, 0)?;
            json!(theme.fg(color, text(args, 1)?)?)
        }
        "bg" => {
            let color: &ThemeBg = text(args, 0)?;
            json!(theme.bg(color, text(args, 1)?)?)
        }
        "bold" => json!(theme.bold(text(args, 0)?)?),
        "italic" => json!(theme.italic(text(args, 0)?)?),
        "underline" => json!(theme.underline(text(args, 0)?)?),
        "inverse" => json!(theme.inverse(text(args, 0)?)?),
        "strikethrough" => json!(theme.strikethrough(text(args, 0)?)?),
        "getFgAnsi" => json!(theme.get_fg_ansi(text(args, 0)?)?),
        "getBgAnsi" => json!(theme.get_bg_ansi(text(args, 0)?)?),
        "getColorMode" => json!(match theme.get_color_mode()? {
            ColorMode::Truecolor => "truecolor",
            ColorMode::Color256 => "256color",
        }),
        "thinkingBorder" => {
            let level = text(args, 0)?
                .parse()
                .map_err(|_| "unknown thinking level")?;
            json!(theme.get_thinking_border_color(level)(text(args, 1)?)?)
        }
        "bashModeBorder" => json!(theme.get_bash_mode_border_color()(text(args, 0)?)?),
        _ => return Err(format!("unknown theme method {method}")),
    })
}

/// Runs one command request: a method call, a host publication or the release of every theme.
fn dispatch(api: &ExtensionAPI, themes: &RefCell<Vec<Theme>>, args: &str) -> ExtensionResult<()> {
    if args == "global" {
        themes.borrow_mut().push(maestro_extensions_wasm::theme()?);
        return Ok(());
    }
    if args == "release" {
        drop(themes.take());
        return Ok(());
    }
    if let Some(control) = args.strip_prefix("publish ") {
        let control: Value = serde_json::from_str(control).map_err(|error| error.to_string())?;
        return api.append_entry("theme-control", Some(control));
    }
    let operation: Operation = serde_json::from_str(args).map_err(|error| error.to_string())?;
    let theme = themes
        .borrow()
        .get(operation.index)
        .cloned()
        .ok_or("missing theme")?;
    let answer = match perform(&theme, &operation.method, &operation.args) {
        Ok(value) => json!({ "ok": value }),
        Err(error) => json!({ "error": error }),
    };
    api.append_entry("theme-answer", Some(answer))
}

/// Registers the renderer that retains every supplied theme and the command that uses them.
pub fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let themes: Rc<RefCell<Vec<Theme>>> = Rc::default();
    let kept = Rc::clone(&themes);
    api.register_message_renderer(
        "theme",
        Rc::new(move |_, _, theme, _| {
            kept.borrow_mut().push(theme);
            Ok(None)
        }),
    )?;
    let log = api.clone();
    api.register_command(
        "theme",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                let (log, themes) = (log.clone(), Rc::clone(&themes));
                Box::pin(async move { dispatch(&log, &themes, &args) })
            }),
        },
    )
}

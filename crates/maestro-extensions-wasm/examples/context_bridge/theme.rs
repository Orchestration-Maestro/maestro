//! Retained renderer themes read the original native publication on every call.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{CallbackEmitter, Component, ComponentView, ExtensionResult, Theme};
use serde_json::{Value, json};

/// Retains a theme independently of the renderer invocation that supplied it.
pub(super) fn view(theme: Theme, method: String) -> Component {
    Component::new(View { theme, method })
}

/// One independently retained theme and the operation the native caller selected.
struct View {
    /// Original live host theme capability.
    theme: Theme,
    /// Operation whose exact result the component returns.
    method: String,
}
impl ComponentView for View {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![
            observe(&self.theme, &self.method, width == 0)?.to_string(),
        ])
    }
    fn handle_input(&self, _: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.theme.set_source_info(None)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Exercises metadata, provenance edits, colors and text decorations through the author API.
fn observe(theme: &Theme, method: &str, missing: bool) -> ExtensionResult<Value> {
    let color = if missing {
        " missing color "
    } else {
        " literal color "
    };
    let text = " literal text\r\n雪 ";
    Ok(match method {
        "name" => json!(theme.name()?),
        "source-path" => json!(theme.source_path()?),
        "source-info" => json!(theme.source_info()?),
        "set-source-info" => {
            let source = serde_json::from_value(json!({"path":"./edited/../theme", "source":" literal source ", "scope":"project", "origin":"package", "baseDir":"./base"})).map_err(|error| error.to_string())?;
            theme.set_source_info(Some(source))?;
            json!(theme.source_info()?)
        }
        "fg" => json!(theme.fg(color, text)?),
        "bg" => json!(theme.bg(color, text)?),
        "bold" => json!(theme.bold(text)?),
        "italic" => json!(theme.italic(text)?),
        "underline" => json!(theme.underline(text)?),
        "inverse" => json!(theme.inverse(text)?),
        "strikethrough" => json!(theme.strikethrough(text)?),
        "get-fg-ansi" => json!(theme.get_fg_ansi(color)?),
        "get-bg-ansi" => json!(theme.get_bg_ansi(color)?),
        "get-color-mode" => json!(match theme.get_color_mode()? {
            maestro_extensions_wasm::ColorMode::Truecolor => "truecolor",
            maestro_extensions_wasm::ColorMode::Color256 => "color256",
        }),
        "bash-mode-border" => json!(theme.get_bash_mode_border_color()(text)?),
        _ => {
            let level = method.strip_prefix("thinking-border:");
            let level = level.ok_or("unknown theme operation")?;
            let level = level.parse().map_err(|_| "unknown thinking level")?;
            json!(theme.get_thinking_border_color(level)(text)?)
        }
    })
}

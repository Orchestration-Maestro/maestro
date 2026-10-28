//! Tool-call summaries share the host's semantic colors and home observation.
use crate::extension::preview;
use maestro_extensions_wasm::{ExtensionResult, Theme};
use serde_json::Value;

/// Chooses a nonempty supplied string or a literal empty-state spelling.
fn text<'a>(args: &'a Value, key: &str, default: &'a str) -> &'a str {
    args[key]
        .as_str()
        .filter(|text| !text.is_empty())
        .unwrap_or(default)
}
/// Retains both accepted path parameter spellings.
fn path<'a>(args: &'a Value, default: &'a str) -> &'a str {
    text(args, "file_path", text(args, "path", default))
}
/// Shortens a home-prefixed authored spelling for display only.
fn shorten(path: &str, home: &str) -> String {
    path.strip_prefix(home)
        .map_or_else(|| path.into(), |suffix| format!("~{suffix}"))
}
/// Formats the built-in tools and a bounded JSON fallback for other names.
pub(super) fn format(
    name: &str,
    args: &Value,
    theme: &Theme,
    home: &str,
) -> ExtensionResult<String> {
    let muted = |text: &str| theme.fg("muted", text);
    let accent = |text: &str| theme.fg("accent", text);
    Ok(match name {
        "bash" => {
            muted("$ ")? + &theme.fg("toolOutput", &preview(text(args, "command", "..."), 60))?
        }
        "read" => read(args, theme, home)?,
        "write" => {
            let lines = args["content"].as_str().unwrap_or("").split('\n').count();
            let mut value = muted("write ")? + &accent(&shorten(path(args, "..."), home))?;
            if lines > 1 {
                value += &theme.fg("dim", &format!(" ({lines} lines)"))?;
            }
            value
        }
        "edit" => muted("edit ")? + &accent(&shorten(path(args, "..."), home))?,
        "ls" => muted("ls ")? + &accent(&shorten(text(args, "path", "."), home))?,
        "find" => {
            muted("find ")?
                + &accent(text(args, "pattern", "*"))?
                + &theme.fg(
                    "dim",
                    &format!(" in {}", shorten(text(args, "path", "."), home)),
                )?
        }
        "grep" => {
            muted("grep ")?
                + &accent(&format!("/{}/", text(args, "pattern", "")))?
                + &theme.fg(
                    "dim",
                    &format!(" in {}", shorten(text(args, "path", "."), home)),
                )?
        }
        _ => {
            accent(name)?
                + &theme.fg(
                    "dim",
                    &format!(
                        " {}",
                        preview(
                            &maestro_json::compact_json(args).map_err(|error| error.to_string())?,
                            50
                        )
                    ),
                )?
        }
    })
}
/// Shows the authored read range without truncating path text.
fn read(args: &Value, theme: &Theme, home: &str) -> ExtensionResult<String> {
    let mut value =
        theme.fg("muted", "read ")? + &theme.fg("accent", &shorten(path(args, "..."), home))?;
    if args.get("offset").is_some() || args.get("limit").is_some() {
        let start = args["offset"].as_f64().unwrap_or(1.0);
        let end = args["limit"]
            .as_f64()
            .map(|limit| start + limit - 1.0)
            .filter(|end| *end != 0.0)
            .map_or_else(String::new, |end| format!("-{end}"));
        value += &theme.fg("warning", &format!(":{start}{end}"))?;
    }
    Ok(value)
}

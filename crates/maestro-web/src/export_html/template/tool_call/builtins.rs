//! Built-in offline card branches.
use super::{INVALID_ARG, format_expandable_output, html, result, values};
use maestro_theme::SyntaxHighlighter;
use serde_json::Value;
use std::{fmt::Write as _, io};

/// Select the built-in display policy without executing a tool.
pub(super) fn render_builtin(
    name: &str,
    args: &Value,
    result: Option<&Value>,
    highlighter: &dyn SyntaxHighlighter,
) -> io::Result<String> {
    match name {
        "ls" => directory(args, result, highlighter),
        "edit" => edit(args, result),
        "write" => write(args, result, highlighter),
        "read" => read(args, result, highlighter),
        _ => bash(args, result, highlighter),
    }
}
/// Prepare a literal shell command and trimmed expandable output.
fn bash(
    args: &Value,
    result: Option<&Value>,
    highlighter: &dyn SyntaxHighlighter,
) -> io::Result<String> {
    let command = values::str(args.get("command"));
    let command = command.map_or_else(
        || INVALID_ARG.to_owned(),
        |s| html(if s.is_empty() { "..." } else { s }),
    );
    let mut body = format!("<div class=\"tool-command\">$ {command}</div>");
    let output = result::get_result_text(result)?;
    let output = output.trim_matches(maestro_app::is_whitespace_scalar);
    if !output.is_empty() {
        body.push_str(&format_expandable_output(output, 5, None, highlighter));
    }
    Ok(body)
}
/// Borrow the nullish-precedence file operand, retaining an invalid present value.
fn path(args: &Value) -> Option<&str> {
    values::str(
        args.get("file_path")
            .filter(|v| !v.is_null())
            .or_else(|| args.get("path")),
    )
}
/// Escape the one lexical viewer path owner, or show the invalid-argument notice.
fn path_html(args: &Value) -> String {
    path(args).map_or_else(
        || INVALID_ARG.to_owned(),
        |s| html(&values::shorten_path(s)),
    )
}
/// File header and untrimmed read output.
fn read(
    args: &Value,
    result: Option<&Value>,
    highlighter: &dyn SyntaxHighlighter,
) -> io::Result<String> {
    let mut title = path_html(args);
    if path(args).is_some() && (args.get("offset").is_some() || args.get("limit").is_some()) {
        let _ = write!(
            title,
            "<span class=\"line-numbers\">{}</span>",
            read_line_range(args)?
        );
    }
    let mut body = format!(
        "<div class=\"tool-header\"><span class=\"tool-name\">read</span> <span class=\"tool-path\">{title}</span></div>"
    );
    body.push_str(&result::render_result_images(result)?);
    let output = result::get_result_text(result)?;
    if !output.is_empty() {
        body.push_str(&format_expandable_output(
            &output,
            10,
            path(args).and_then(values::get_language_from_path),
            highlighter,
        ));
    }
    Ok(body)
}

/// Delegate numeric arithmetic and keep malformed display operands literal.
fn read_line_range(args: &Value) -> io::Result<String> {
    let numeric = |value: Option<&Value>| value.is_none_or(|v| v.is_null() || v.is_number());
    if numeric(args.get("offset")) && numeric(args.get("limit")) {
        return Ok(maestro_app::format_read_line_range(args));
    }
    let default = Value::from(1);
    let start = args
        .get("offset")
        .filter(|v| !v.is_null())
        .unwrap_or(&default);
    let start = result::display_value(start)?;
    let end = args.get("limit").map(result::display_value).transpose()?;
    let tail = end
        .filter(|s| !s.is_empty())
        .map_or_else(String::new, |s| format!("-{s}"));
    Ok(html(&format!(":{start}{tail}")))
}

/// Content previews stay untrimmed while the returned write notice is one literal block.
fn write(
    args: &Value,
    result: Option<&Value>,
    highlighter: &dyn SyntaxHighlighter,
) -> io::Result<String> {
    let path = path_html(args);
    let content = values::str(args.get("content"));
    let count = content.map_or(0, |s| s.split('\n').count());
    let count = if count > 10 {
        format!(" <span class=\"line-count\">({count} lines)</span>")
    } else {
        String::new()
    };
    let mut body = format!(
        "<div class=\"tool-header\"><span class=\"tool-name\">write</span> <span class=\"tool-path\">{path}</span>{count}</div>"
    );
    match content {
        None => {
            body.push_str(
                "<div class=\"tool-error\">[invalid content arg - expected string]</div>",
            );
        }
        Some("") => {}
        Some(content) => body.push_str(&format_expandable_output(
            content,
            10,
            self::path(args).and_then(values::get_language_from_path),
            highlighter,
        )),
    }
    let output = result::get_result_text(result)?;
    let output = output.trim_matches(maestro_app::is_whitespace_scalar);
    if !output.is_empty() {
        let _ = write!(
            body,
            "<div class=\"tool-output\"><div>{}</div></div>",
            html(output)
        );
    }
    Ok(body)
}

/// Diff rows retain literal tabs, leading roles and reached malformed-diff errors.
fn edit(args: &Value, result: Option<&Value>) -> io::Result<String> {
    let path = path_html(args);
    let mut body = format!(
        "<div class=\"tool-header\"><span class=\"tool-name\">edit</span> <span class=\"tool-path\">{path}</span></div>"
    );
    if let Some(diff) = result
        .map(|r| &r["details"]["diff"])
        .filter(|v| maestro_app::is_truthy(v))
    {
        let diff = diff.as_str().ok_or_else(super::invalid)?;
        body.push_str("<div class=\"tool-diff\">");
        for line in diff.split('\n') {
            let role = match line.as_bytes().first() {
                Some(b'+') => "added",
                Some(b'-') => "removed",
                _ => "context",
            };
            let _ = write!(
                body,
                "<div class=\"diff-{role}\">{}</div>",
                html(&maestro_app::replace_tabs(line))
            );
        }
        body.push_str("</div>");
    } else {
        let output = result::get_result_text(result)?;
        let output = output.trim_matches(maestro_app::is_whitespace_scalar);
        if !output.is_empty() {
            let _ = write!(
                body,
                "<div class=\"tool-output\"><pre>{}</pre></div>",
                html(output)
            );
        }
    }
    Ok(body)
}

/// Directory cards distinguish an omitted limit from every supplied display kind.
fn directory(
    args: &Value,
    result: Option<&Value>,
    highlighter: &dyn SyntaxHighlighter,
) -> io::Result<String> {
    let mut title = values::str(args.get("path")).map_or_else(
        || INVALID_ARG.to_owned(),
        |s| html(&values::shorten_path(if s.is_empty() { "." } else { s })),
    );
    if let Some(limit) = args.get("limit") {
        let _ = write!(
            title,
            " <span class=\"line-count\">(limit {})</span>",
            html(&result::display_value(limit)?)
        );
    }
    let mut body = format!(
        "<div class=\"tool-header\"><span class=\"tool-name\">ls</span> <span class=\"tool-path\">{title}</span></div>"
    );
    let output = result::get_result_text(result)?;
    let output = output.trim_matches(maestro_app::is_whitespace_scalar);
    if !output.is_empty() {
        body.push_str(&format_expandable_output(output, 20, None, highlighter));
    }
    Ok(body)
}

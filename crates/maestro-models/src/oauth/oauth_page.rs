//! Callback HTML with embedded brand data.
use crate::DiagnosticErrorInfo;
use maestro_brand::{BrandError, BrandMode, BrandPresentation, PackData, font_stack, role_color};
use std::fmt::Write as _;

/// Embedded identity values.
const BRAND: &str = include_str!("../../../../assets/brand/brand.json");
/// Embedded identity mark.
const MARK: &str = include_str!("../../../../assets/brand/mark.svg");
/// Callback layout; slots are title, style, mark, heading, message and details.
const TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{}</title>
  <style>
    :root { {} }
    * { box-sizing: border-box; }
    html { color-scheme: dark; }
    body {
      margin: 0;
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 24px;
      background: var(--page-bg);
      color: var(--text);
      font-family: var(--font-body);
      text-align: center;
    }
    main {
      width: 100%;
      max-width: 560px;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
    }
    .logo {
      width: 72px;
      height: 72px;
      display: block;
      margin-bottom: 24px;
    }
    .logo svg { width: 100%; height: 100%; }
    h1 {
      font-family: var(--font-display);
      margin: 0 0 10px;
      font-size: 28px;
      line-height: 1.15;
      font-weight: 650;
      color: var(--text);
    }
    p {
      margin: 0;
      line-height: 1.7;
      color: var(--text-dim);
      font-size: 15px;
    }
    .details {
      margin-top: 16px;
      font-family: var(--font-mono);
      font-size: 13px;
      color: var(--text-dim);
      white-space: pre-wrap;
      word-break: break-word;
    }
  </style>
</head>
<body>
  <main>
    <div class="logo">{}</div>
    <h1>{}</h1>
    <p>{}</p>
    {}
  </main>
</body>
</html>"#;

/// Render the successful callback document with escaped caller text.
///
/// # Errors
/// Returns an error if required embedded identity data cannot be resolved.
pub fn oauth_success_html(message: &str) -> Result<String, DiagnosticErrorInfo> {
    render_page("Authentication successful", message, None, BRAND, MARK)
}

/// Render the failed callback document; absent or empty details omit the details block.
///
/// # Errors
/// Returns an error if required embedded identity data cannot be resolved.
pub fn oauth_error_html(
    message: &str,
    details: Option<&str>,
) -> Result<String, DiagnosticErrorInfo> {
    render_page("Authentication failed", message, details, BRAND, MARK)
}

/// Resolve identity separately from caller text and fill the static layout slots.
fn render_page(
    title: &str,
    message: &str,
    details: Option<&str>,
    pack: &str,
    svg: &str,
) -> Result<String, DiagnosticErrorInfo> {
    let pack = PackData::parse(pack).map_err(|error| diagnostic(&error))?;
    let brand = pack
        .presentation(BrandMode::Dark)
        .map_err(|error| diagnostic(&error))?;
    let style = brand_style(&brand).map_err(|error| diagnostic(&error))?;
    let mark = brand.fill_mark(svg).map_err(|error| diagnostic(&error))?;
    let message = escape_html(message);
    let details = details
        .filter(|value| !value.is_empty())
        .map_or_else(String::new, |value| {
            format!("<div class=\"details\">{}</div>", escape_html(value))
        });
    let values = [title, &style, &mark, title, &message, &details, ""];
    let mut html = String::new();
    for (part, value) in TEMPLATE.split("{}").zip(values) {
        html.push_str(part);
        html.push_str(value);
    }
    Ok(html)
}

/// Resolve the callback's own color and font aliases from the shared presentation.
fn brand_style(brand: &BrandPresentation<'_>) -> Result<String, BrandError> {
    let mut style = String::new();
    for role in ["text", "muted", "background"] {
        let color = role_color(&brand.colors, role)?;
        let _ = write!(style, "--{role}: {color}; ");
    }
    style.push_str("--text-dim: var(--muted); --page-bg: var(--background); ");
    for role in ["display", "body", "mono"] {
        let font = brand
            .fonts
            .get(role)
            .ok_or_else(|| BrandError::MissingFont(role.to_owned()))?;
        let _ = write!(style, "--font-{role}: {}; ", font_stack(font));
    }
    Ok(style)
}

/// Escape literal caller text without rescanning inserted entities.
fn escape_html(value: &str) -> String {
    maestro_text::markup::escape(value, maestro_text::markup::EscapeMode::HtmlDecimal)
}

/// Retain the native parse or resolution message without supplied diagnostic fields.
fn diagnostic(error: &BrandError) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        message: error.to_string(),
        name: None,
        stack: None,
        code: None,
    }
}

#[cfg(test)]
mod tests;

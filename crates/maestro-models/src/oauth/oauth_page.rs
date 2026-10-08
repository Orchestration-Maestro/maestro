//! Callback HTML with embedded brand data.
use crate::DiagnosticErrorInfo;
use serde::{Deserialize, de::Error as _};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
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

/// Consumed identity fields; unescaped wordmark and primary font text can borrow the pack.
#[derive(Deserialize)]
struct Brand<'a> {
    /// Title substitution for the mark.
    #[serde(borrow)]
    wordmark: Cow<'a, str>,
    /// Palette aliases.
    #[serde(borrow)]
    palette: BTreeMap<Cow<'a, str>, Cow<'a, str>>,
    /// Font stacks.
    #[serde(borrow)]
    fonts: Fonts<'a>,
    /// Semantic color modes.
    #[serde(borrow)]
    modes: Modes<'a>,
}
/// The three font roles consumed by callback HTML.
#[derive(Deserialize)]
struct Fonts<'a> {
    /// Heading font.
    #[serde(borrow)]
    display: Font<'a>,
    /// Paragraph font.
    #[serde(borrow)]
    body: Font<'a>,
    /// Details font.
    #[serde(borrow)]
    mono: Font<'a>,
}
/// A family and ordered fallback stack.
#[derive(Deserialize)]
struct Font<'a> {
    /// Primary family name.
    #[serde(borrow)]
    family: Cow<'a, str>,
    /// Ordered fallback names.
    #[serde(borrow)]
    fallbacks: Vec<Cow<'a, str>>,
}
/// Consumed color mode.
#[derive(Deserialize)]
struct Modes<'a> {
    /// Callback pages use the dark mode.
    #[serde(borrow)]
    dark: Colors<'a>,
}
/// Semantic aliases for one mode.
#[derive(Deserialize)]
struct Colors<'a> {
    /// Role-to-palette mapping.
    #[serde(borrow)]
    colors: BTreeMap<Cow<'a, str>, Cow<'a, str>>,
}

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
    let brand: Brand<'_> = serde_json::from_str(pack).map_err(|error| diagnostic(&error))?;
    let style = brand_style(&brand, svg).map_err(|error| diagnostic(&error))?;
    let mark = svg.replace("{{wordmark}}", &escape_html(&brand.wordmark));
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

/// Resolve page roles and the roles actually referenced by the supplied mark.
fn brand_style(brand: &Brand<'_>, svg: &str) -> Result<String, serde_json::Error> {
    let mut roles = BTreeSet::from(["text", "muted", "background"]);
    roles.extend(
        svg.split("var(--")
            .skip(1)
            .filter_map(|part| part.split_once(')').map(|(role, _)| role)),
    );
    let mut style = String::new();
    for role in roles {
        let alias = brand
            .modes
            .dark
            .colors
            .get(role)
            .ok_or_else(|| serde_json::Error::custom(format!("missing color role `{role}`")))?;
        let color = brand
            .palette
            .get(alias.as_ref())
            .ok_or_else(|| serde_json::Error::custom(format!("missing palette key `{alias}`")))?;
        let _ = write!(style, "--{role}: {color}; ");
    }
    style.push_str("--text-dim: var(--muted); --page-bg: var(--background); ");
    for (role, font) in [
        ("display", &brand.fonts.display),
        ("body", &brand.fonts.body),
        ("mono", &brand.fonts.mono),
    ] {
        let stack = font_stack(font);
        let _ = write!(style, "--font-{role}: {stack}; ");
    }
    Ok(style)
}

/// Quote the primary family and retain generic fallback names in their supplied order.
fn font_stack(font: &Font<'_>) -> String {
    let quote = |name: &str| {
        format!(
            "\"{}\"",
            name.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('<', "\\3c ")
        )
    };
    std::iter::once(quote(&font.family))
        .chain(font.fallbacks.iter().map(|name| match name.as_ref() {
            "serif" | "sans-serif" | "monospace" | "system-ui" | "ui-monospace" | "ui-serif"
            | "ui-sans-serif" | "ui-rounded" | "cursive" | "fantasy" => name.to_string(),
            other => quote(other),
        }))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Escape literal caller text without rescanning inserted entities.
fn escape_html(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Retain the native parse or resolution message without supplied diagnostic fields.
fn diagnostic(error: &serde_json::Error) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        message: error.to_string(),
        name: None,
        stack: None,
        code: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_pages_use_replaceable_brand_data() {
        let signal = include_str!("../../../../assets/brand/packs/signal.json");
        let flat = include_str!("../../../../assets/brand/mark-flat.svg");
        for (pack, svg, fonts) in [
            (BRAND, MARK, ["Barlow Condensed", "Barlow"]),
            (signal, flat, ["Space Grotesk", "Space Grotesk"]),
        ] {
            let page =
                render_page("Authentication successful", "body witness", None, pack, svg).unwrap();
            let data: serde_json::Value = serde_json::from_str(pack).unwrap();
            let roles = if svg == MARK {
                vec![
                    "text",
                    "muted",
                    "background",
                    "mark-ring",
                    "mark-m",
                    "mark-node",
                    "mark-star",
                    "mark-live",
                ]
            } else {
                vec![
                    "text",
                    "muted",
                    "background",
                    "mark-ring",
                    "mark-m",
                    "mark-node",
                    "mark-star",
                ]
            };
            for role in roles {
                let alias = data["modes"]["dark"]["colors"][role].as_str().unwrap();
                assert!(page.contains(&format!(
                    "--{role}: {};",
                    data["palette"][alias].as_str().unwrap()
                )));
            }
            for (role, family) in [("display", fonts[0]), ("body", fonts[1])] {
                assert!(page.contains(&format!(
                    "--font-{role}: \"{family}\", system-ui, sans-serif;"
                )));
            }
            assert!(page.contains("--font-mono: \"JetBrains Mono\", ui-monospace, monospace;"));
            assert!(page.contains(&svg.replace("{{wordmark}}", "Maestro")));
            assert!(page.contains("<h1>Authentication successful</h1>"));
        }
        check_alternate_brand(signal);
    }

    /// Check changed mark, wordmark and font stacks through the same renderer.
    fn check_alternate_brand(signal: &str) {
        let mut alternate: serde_json::Value = serde_json::from_str(signal).unwrap();
        alternate["wordmark"] = serde_json::json!("&<new>\"'");
        alternate["fonts"]["display"]["family"] = serde_json::json!("Alternate Heading");
        alternate["fonts"]["body"]["family"] = serde_json::json!("Alternate Body");
        alternate["fonts"]["mono"]["fallbacks"] = serde_json::json!(["monospace", "ui-monospace"]);
        let page = render_page(
            "Authentication failed",
            "body witness",
            Some("details witness"),
            &alternate.to_string(),
            "<svg><title>{{wordmark}}</title><path fill=\"var(--mark-live)\" d=\"M0 0\"/></svg>",
        )
        .unwrap();
        for value in [
            "&amp;&lt;new&gt;&quot;&#39;",
            "\"Alternate Heading\"",
            "\"Alternate Body\"",
            "--font-mono: \"JetBrains Mono\", monospace, ui-monospace;",
            "--mark-live: #B6F05A;",
            "<path fill=\"var(--mark-live)\" d=\"M0 0\"/>",
            "<h1>Authentication failed</h1>",
            "<p>body witness</p>",
            "<div class=\"details\">details witness</div>",
        ] {
            assert!(page.contains(value), "{value}");
        }
    }
    #[test]
    fn oauth_pages_report_invalid_brand_data() {
        let native = serde_json::from_str::<Brand<'_>>("{")
            .err()
            .unwrap()
            .to_string();
        let error = render_page("Authentication failed", "test", None, "{", MARK).unwrap_err();
        assert_eq!(
            error,
            DiagnosticErrorInfo {
                message: native,
                name: None,
                stack: None,
                code: None
            }
        );
        for (kind, expected) in [
            ("field", "missing field `family`"),
            ("role", "missing color role `text`"),
            ("palette", "missing palette key `dangling`"),
        ] {
            let mut pack: serde_json::Value = serde_json::from_str(BRAND).unwrap();
            match kind {
                "field" => {
                    pack["fonts"]["body"]
                        .as_object_mut()
                        .unwrap()
                        .remove("family");
                }
                "role" => {
                    pack["modes"]["dark"]["colors"]
                        .as_object_mut()
                        .unwrap()
                        .remove("text");
                }
                _ => pack["modes"]["dark"]["colors"]["text"] = serde_json::json!("dangling"),
            }
            let error = render_page(
                "Authentication failed",
                "test",
                None,
                &pack.to_string(),
                MARK,
            )
            .unwrap_err();
            assert!(error.message.starts_with(expected), "{}", error.message);
            assert_eq!((error.name, error.code, error.stack), (None, None, None));
        }
    }
}

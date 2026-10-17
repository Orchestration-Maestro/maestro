//! HTML detection keeps surface membership separate from the native painter.
#[cfg(test)]
pub mod support;
use maestro_theme::{HighlightProfile, SyntaxSpan, ThemeColor, highlight_auto};
use serde::Deserialize;
use support::highlighting::{Scripted, decode, native};

/// One authored label and independently recorded canonical routes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Route {
    /// Authored label, never trimmed or split.
    label: Option<String>,
    /// Exported-session canonical painter label.
    viewer: String,
    /// Browser canonical painter label.
    browser: String,
}

/// Both distinct surface policies.
const PROFILES: [HighlightProfile; 2] =
    [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock];
/// Original input used for the recorded caller operations.
const ROUTE_CODE: &str = "const n = \"<&>'\\\"\";\r\n";
/// Every canonical language selected by the recorded operations.
const ROUTE_LANGUAGES: &[&str] = &[
    "cpp",
    "javascript",
    "xml",
    "html",
    "c",
    "ini",
    "json",
    "bash",
    "kotlin",
];

/// Independent canonical route observations.
#[cfg(test)]
fn routes() -> Vec<Route> {
    serde_json::from_str(include_str!(
        "fixtures/automatic_highlighting/caller-routes.json"
    ))
    .unwrap()
}

/// The expected route for one surface.
fn expected(route: &Route, profile: HighlightProfile) -> &str {
    match profile {
        HighlightProfile::ExportViewer => &route.viewer,
        HighlightProfile::CodeBlock => &route.browser,
    }
}

#[test]
fn automatic_html_uses_each_surface_registered_label_policy() {
    let routes = routes();
    assert_eq!(routes.len(), 14);
    for route in &routes {
        for profile in PROFILES {
            let engine = Scripted::new(
                ROUTE_LANGUAGES,
                Ok(vec![SyntaxSpan {
                    range: 0..ROUTE_CODE.len(),
                    color: None,
                }]),
            );
            let html =
                highlight_auto(&engine, ROUTE_CODE, route.label.as_deref(), profile).unwrap();
            assert_eq!(
                html,
                "const n = &quot;&lt;&amp;&gt;&#x27;\\&quot;&quot;;\r\n"
            );
            let label = expected(route, profile);
            assert_eq!(
                engine.calls(),
                [format!("supports:{label}"), format!("highlight:{label}")]
            );
        }
    }
}

#[test]
fn automatic_html_preserves_text_and_existing_token_classes() {
    for code in [
        "",
        "\n",
        "a\n\n\n",
        "x\r\ny\r\n",
        "\tx\t",
        "const n = 42; // 雪 💡\r\n",
        "const x = \"<&>\" + '\\'' + \"&amp; &lt;\";\n",
    ] {
        for profile in PROFILES {
            let html = highlight_auto(native(), code, Some("javascript"), profile).unwrap();
            assert_eq!(decode(&html), code);
        }
    }
    let html =
        highlight_auto(native(), "const n = 42;", None, HighlightProfile::CodeBlock).unwrap();
    assert!(html.contains("class=\"hljs-number\""));
    let code = "<&>\"' &amp;\r\n\t💡";
    let engine = Scripted::new(
        &["javascript"],
        Ok(vec![SyntaxSpan {
            range: 0..code.len(),
            color: Some(ThemeColor::SyntaxKeyword),
        }]),
    );
    let html = highlight_auto(
        &engine,
        code,
        Some("javascript"),
        HighlightProfile::CodeBlock,
    )
    .unwrap();
    assert_eq!(
        html,
        "<span class=\"hljs-keyword\">&lt;&amp;&gt;&quot;&#x27; &amp;amp;\r\n\t💡</span>"
    );
    assert_eq!(decode(&html), code);
    assert_eq!(
        engine.calls(),
        ["supports:javascript", "highlight:javascript"]
    );
}

#[test]
fn automatic_html_propagates_painter_errors() {
    for route in routes() {
        for profile in PROFILES {
            let engine = Scripted::new(ROUTE_LANGUAGES, Err("controlled engine failure"));
            let error =
                highlight_auto(&engine, ROUTE_CODE, route.label.as_deref(), profile).unwrap_err();
            assert_eq!(error.to_string(), "controlled engine failure");
            let label = expected(&route, profile);
            assert_eq!(
                engine.calls(),
                [format!("supports:{label}"), format!("highlight:{label}")]
            );
        }
    }
}

#[test]
fn automatic_plain_result_is_escaped_without_painter_calls() {
    for profile in PROFILES {
        for code in ["", "   ", "\n\r\n", "\u{feff}", "\u{85}"] {
            let engine = Scripted::failing();
            assert_eq!(highlight_auto(&engine, code, None, profile).unwrap(), code);
            assert!(engine.calls().is_empty());
        }
        let engine = Scripted::new(&[], Err("must not paint"));
        assert_eq!(
            highlight_auto(&engine, ROUTE_CODE, None, profile).unwrap(),
            "const n = &quot;&lt;&amp;&gt;&#x27;\\&quot;&quot;;\r\n"
        );
        assert_eq!(
            engine.calls(),
            [format!(
                "supports:{}",
                if profile == HighlightProfile::ExportViewer {
                    "cpp"
                } else {
                    "javascript"
                }
            )]
        );
    }
    let engine = Scripted::new(&[], Err("must not paint"));
    assert_eq!(
        highlight_auto(
            &engine,
            "<&>",
            Some("javascript"),
            HighlightProfile::CodeBlock
        )
        .unwrap(),
        "&lt;&amp;&gt;"
    );
    assert_eq!(engine.calls(), ["supports:javascript"]);
}

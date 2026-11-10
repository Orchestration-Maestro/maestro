//! Window-title template and sanitation behavior.

use super::super::titles::{
    WindowTitlePart, WindowTitleTemplate, WindowTitleToken, default_window_title,
    sanitize_window_title_text, window_title_diagnostics,
};

#[test]
fn window_title_sanitization_bounds_scalars() {
    assert_eq!(
        sanitize_window_title_text("  Maestro\u{1b} api\u{7}\n\u{9c}  ").as_deref(),
        Some("Maestro api")
    );
    assert_eq!(sanitize_window_title_text("\u{7}\n\u{9c}"), None);
    for count in [199, 200, 201] {
        assert_eq!(
            sanitize_window_title_text(&"界".repeat(count)),
            Some("界".repeat(count.min(200)))
        );
    }
    assert_eq!(
        sanitize_window_title_text(&format!(" {}x", "界".repeat(199))),
        Some("界".repeat(199)),
        "scalar limiting precedes trimming"
    );
}

#[test]
fn window_title_default_template_decodes() {
    let value = default_window_title();
    assert_eq!(value, "{hostname}: {workspace}");
    assert_eq!(
        WindowTitleTemplate::parse(&value).unwrap().unwrap().parts(),
        [
            WindowTitlePart::Token(WindowTitleToken::Hostname),
            WindowTitlePart::Literal(": ".into()),
            WindowTitlePart::Token(WindowTitleToken::Workspace),
        ]
    );
}

#[test]
fn window_title_empty_template_disables_output() {
    assert_eq!(WindowTitleTemplate::parse(""), Ok(None));
    assert_eq!(window_title_diagnostics(""), None);
}

#[test]
fn window_title_token_names_allow_padding() {
    let template = WindowTitleTemplate::parse("  { tab } \t").unwrap().unwrap();
    assert_eq!(
        template.parts(),
        [
            WindowTitlePart::Literal("  ".into()),
            WindowTitlePart::Token(WindowTitleToken::Tab),
            WindowTitlePart::Literal(" \t".into()),
        ]
    );
}

#[test]
fn window_title_errors_name_the_invalid_brace() {
    for (value, error) in [
        ("{hostname", "has an unclosed '{'"),
        ("a } b", "has an unmatched '}'"),
        ("{session}", "has unknown token '{session}'"),
    ] {
        assert_eq!(WindowTitleTemplate::parse(value).unwrap_err(), error);
        assert_eq!(
            window_title_diagnostics(value),
            Some(format!(
                "multiplexer.windowTitle {error}; leaving the outer terminal title alone"
            ))
        );
    }
}

#[test]
fn window_title_parser_keeps_tokens_and_escapes() {
    let template =
        WindowTitleTemplate::parse("{hostname}: {workspace} {{a}} {tab}/{pane}/{terminal_title}")
            .unwrap()
            .unwrap();
    assert_eq!(
        template.parts(),
        [
            WindowTitlePart::Token(WindowTitleToken::Hostname),
            WindowTitlePart::Literal(": ".into()),
            WindowTitlePart::Token(WindowTitleToken::Workspace),
            WindowTitlePart::Literal(" {a} ".into()),
            WindowTitlePart::Token(WindowTitleToken::Tab),
            WindowTitlePart::Literal("/".into()),
            WindowTitlePart::Token(WindowTitleToken::Pane),
            WindowTitlePart::Literal("/".into()),
            WindowTitlePart::Token(WindowTitleToken::TerminalTitle),
        ]
    );
    for token in [
        WindowTitleToken::Hostname,
        WindowTitleToken::Workspace,
        WindowTitleToken::Tab,
        WindowTitleToken::Pane,
        WindowTitleToken::TerminalTitle,
    ] {
        assert!(template.uses(token));
    }
    let literal = WindowTitleTemplate::parse("plain").unwrap().unwrap();
    assert!(!literal.uses(WindowTitleToken::Hostname));
}

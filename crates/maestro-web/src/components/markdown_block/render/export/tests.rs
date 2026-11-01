//! Export walking needs no transcript indexing state.
use super::super::{Policy, Prepared, Writer};
use maestro_app::markdown;

#[test]
fn export_writer_needs_no_inline_context() {
    let highlighter = maestro_theme::SyntectHighlighter::new().unwrap();
    let document = markdown::parse_with_options(
        "**bold** [go](/docs)".to_owned(),
        markdown::MarkedOptions::Export,
    );
    let mut writer = Writer {
        policy: Policy::Export(&highlighter),
        prepared: Prepared::default(),
    };
    let mut html = String::new();
    writer.blocks(&document.nodes, None, &mut html);
    assert_eq!(
        html,
        "<p><strong>bold</strong> <a href=\"/docs\">go</a></p>\n"
    );
    let cases: Vec<ContainerExport> =
        serde_json::from_str(include_str!("../../fixtures/container_export.json")).unwrap();
    let mut failures = Vec::new();
    for case in cases {
        let document =
            markdown::parse_with_options(case.input.clone(), markdown::MarkedOptions::Export);

        let mut writer = Writer {
            policy: Policy::Export(&highlighter),
            prepared: Prepared::default(),
        };
        let mut html = String::new();
        writer.blocks(&document.nodes, None, &mut html);
        if html != case.html {
            failures.push(format!("{:?}: {html:?} != {:?}", case.input, case.html));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// HTML recorded through the standalone document renderer without transcript context.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerExport {
    /// Complete original source supplied to the document renderer.
    input: String,
    /// Complete returned HTML.
    html: String,
}

#[test]
fn export_images_keep_literal_alt_bytes() {
    let highlighter = maestro_theme::SyntectHighlighter::new().unwrap();
    let cases: Vec<ContainerExport> =
        serde_json::from_str(include_str!("../../fixtures/source_image_export.json")).unwrap();
    assert_eq!(cases.len(), 6);
    for case in cases {
        assert_eq!(
            super::prepare_export(&case.input, &highlighter),
            case.html,
            "{:?}",
            case.input
        );
    }
}

#[test]
fn export_targets_keep_only_allowed_schemes() {
    let highlighter = maestro_theme::SyntectHighlighter::new().unwrap();
    for (source, html) in [
        ("[a](<java\u{1}script:alert(1)>)", "<p>a</p>\n"),
        ("[a](<jav\u{1f}ascript:alert(1)>)", "<p>a</p>\n"),
        ("[a](java&#1;script:alert(1))", "<p>a</p>\n"),
        ("![i](vbscript:x)", "<p>i</p>\n"),
        ("![i](<vb\u{7f}script:x>)", "<p>i</p>\n"),
        ("[a](custom:thing)", "<p>a</p>\n"),
        ("![i](data:image/png;base64,AAAA)", "<p>i</p>\n"),
        (
            "[a](https://a.b/c)",
            "<p><a href=\"https://a.b/c\">a</a></p>\n",
        ),
        (
            "[a](HTTP://a.b/c)",
            "<p><a href=\"HTTP://a.b/c\">a</a></p>\n",
        ),
        (
            "[a](mailto:x@y.z)",
            "<p><a href=\"mailto:x@y.z\">a</a></p>\n",
        ),
        ("[a](tel:+1)", "<p><a href=\"tel:+1\">a</a></p>\n"),
        ("[a](ftp://h/f)", "<p><a href=\"ftp://h/f\">a</a></p>\n"),
        ("[a](/relative)", "<p><a href=\"/relative\">a</a></p>\n"),
        ("[a](#top)", "<p><a href=\"#top\">a</a></p>\n"),
        (
            "[a](<https://a.b/\u{1}c>)",
            "<p><a href=\"https://a.b/c\">a</a></p>\n",
        ),
        (
            "![i](https://a.b/i.png)",
            "<p><img src=\"https://a.b/i.png\" alt=\"i\"></p>\n",
        ),
    ] {
        assert_eq!(
            super::prepare_export(source, &highlighter),
            html,
            "{source:?}"
        );
    }
}

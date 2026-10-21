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

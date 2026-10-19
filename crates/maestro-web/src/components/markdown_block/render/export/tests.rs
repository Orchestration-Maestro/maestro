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
}

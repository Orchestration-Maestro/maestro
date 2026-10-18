//! Operand preservation at the source-admission boundary.
use super::escape_tags;

/// Escapes, definitions and native operands never enter HTML neutralization.
#[test]
fn export_admission_preserves_parser_owned_operands() {
    for source in [
        "\\<",
        "\\<div>",
        "\\<!DOCTYPE html>",
        "[r][r]\n\n[r]: <https://safe.test>",
        "[r][r]\n\n[r]: </docs>",
        "![alt][r]\n\n[r]: </image.png>",
        "[a](</docs> \"<title>\") ![alt](</image.png>)",
        "`<tag>`",
        "    <tag>\n",
        "<https://safe.test/a> <a@example.test>",
    ] {
        assert_eq!(escape_tags(source.to_owned(), true), source);
    }
    let prepared = escape_tags("<div>\n```text\n<tag>\n```\n</div>".to_owned(), true);
    let document = maestro_app::markdown::parse(prepared);
    assert!(document.nodes.iter().any(|node| matches!(
        &node.kind,
        maestro_app::markdown::Kind::CodeBlock(code, _) if code == "<tag>\n"
    )));
}

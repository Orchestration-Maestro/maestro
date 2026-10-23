//! The preview entry points and the XML declaration option.
use super::{package, preview_case};
use crate::docx_preview::parse_async;

#[test]
fn xml_declaration_option_reaches_the_package_reader() {
    let trimmed = preview_case("xml-declaration-trimmed").options.options();
    assert!(parse_async(&package("xml-declaration-trimmed"), trimmed).is_ok());
    let kept = preview_case("xml-declaration-kept").options.options();
    assert!(!kept.trim_xml_declaration);
    assert!(parse_async(&package("xml-declaration-kept"), kept).is_err());
}

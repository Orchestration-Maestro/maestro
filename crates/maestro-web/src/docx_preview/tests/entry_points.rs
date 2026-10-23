//! The preview entry points, the XML declaration option and the resource URL policy.
use super::{markup, package, preview_case};
use crate::docx_preview::{Options, parse_async};

#[test]
fn xml_declaration_option_reaches_the_package_reader() {
    let trimmed = preview_case("xml-declaration-trimmed").options.options();
    assert!(parse_async(&package("xml-declaration-trimmed"), trimmed).is_ok());
    let kept = preview_case("xml-declaration-kept").options.options();
    assert!(!kept.trim_xml_declaration);
    assert!(parse_async(&package("xml-declaration-kept"), kept).is_err());
}

/// The URL kind of images, picture bullets and fonts follows the options the package was read
/// with, not the options of a later rendering.
#[test]
fn resource_urls_follow_the_parse_options() {
    let options = |base64: bool| {
        let mut options = Options::default();
        options.resources.use_base64_url = base64;
        options
    };
    let (data_urls, object_urls) = (options(true), options(false));
    for fixture in ["image-inline", "numbering-picture-bullet", "embedded-fonts"] {
        let names_data_urls = |parsed: &Options, rendered: &Options| {
            let preview = parse_async(&package(fixture), parsed.clone()).unwrap();
            let rendering = preview.render(rendered);
            rendering
                .body
                .iter()
                .chain(&rendering.resource_styles)
                .map(markup)
                .any(|element| element.contains("data:"))
        };
        assert!(
            names_data_urls(&data_urls, &object_urls),
            "{fixture} keeps the reading policy"
        );
        assert!(
            !names_data_urls(&object_urls, &data_urls),
            "{fixture} ignores the rendering policy"
        );
    }
}

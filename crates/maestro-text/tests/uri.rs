//! URI component encoding of authored text.

use maestro_text::uri::encode_uri_component;

#[test]
fn a_uri_component_keeps_the_punctuation_left_literal() {
    assert_eq!(encode_uri_component("a b/c(d)!'~*"), "a%20b%2Fc(d)!'~*");
}

#[test]
fn a_uri_component_encodes_every_non_ascii_byte() {
    assert_eq!(encode_uri_component("é"), "%C3%A9");
}

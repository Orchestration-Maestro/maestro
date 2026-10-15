//! URI component encoding shared by outgoing requests.
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// URI component punctuation left literal.
const COMPONENT_ENCODING: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// Encode the authored UTF-8 target as a URI component.
pub(crate) fn encode_uri_component(text: &str) -> String {
    utf8_percent_encode(text, COMPONENT_ENCODING).to_string()
}

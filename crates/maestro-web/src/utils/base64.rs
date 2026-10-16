//! Browser-compatible standard base64 framing.
use base64::{
    Engine as _, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use std::io;
/// Decode complete bytes, accepting only browser ASCII whitespace.
pub(crate) fn decode(base64: &str) -> io::Result<Vec<u8>> {
    let framed: Vec<_> = base64
        .bytes()
        .filter(|b| !matches!(b, b'\t' | b'\n' | 12 | b'\r' | b' '))
        .collect();
    if framed.contains(&b'=') && framed.len() % 4 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete base64 padding",
        ));
    }
    let decoder = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    decoder
        .decode(framed)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

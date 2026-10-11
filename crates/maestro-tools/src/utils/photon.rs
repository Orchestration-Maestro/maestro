//! Static image type and shared byte decoding.

use base64::{
    Engine as _, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

/// Native decoded RGBA image representation.
pub use photon_rs::PhotonImage as PhotonImageType;

/// Decode padded or unpadded base64 with URL-safe spelling and ASCII whitespace.
pub(super) fn decode_image(data: &str) -> Option<PhotonImageType> {
    let normalized: String = data
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    let config = GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let bytes = GeneralPurpose::new(&alphabet::STANDARD, config)
        .decode(normalized)
        .ok()?;
    let image = photon_rs::native::open_image_from_bytes(&bytes).ok()?;
    Some(super::exif_orientation::apply_exif_orientation(
        image, &bytes,
    ))
}

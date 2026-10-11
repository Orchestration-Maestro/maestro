//! Supported inline-image MIME detection from bounded native file samples.

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::File,
    io::{self, Read},
};

/// Detect supported image content from one read of at most 4100 bytes.
///
/// File extensions are ignored. Opening and reading errors retain their native identity.
///
/// # Errors
/// Returns the original file opening or reading error.
#[cfg(not(target_arch = "wasm32"))]
pub fn detect_supported_image_mime_type_from_file(
    file_path: &str,
) -> io::Result<Option<&'static str>> {
    let mut file = File::open(file_path)?;
    let mut sample = [0; 4100];
    let count = file.read(&mut sample)?;
    Ok(sniff(&sample[..count]))
}

/// Select supported byte signatures independently of pixel decoding.
#[cfg(not(target_arch = "wasm32"))]
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let bytes = strip_prefixes(bytes)?;
    if bytes.starts_with(b"GIF") {
        return Some("image/gif");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return (bytes.get(3) != Some(&0xf7)).then_some("image/jpeg");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png(bytes);
    }
    None
}

/// Inspect sampled PNG chunk headers, distinguishing animation before image data.
#[cfg(not(target_arch = "wasm32"))]
fn png(bytes: &[u8]) -> Option<&'static str> {
    let mut offset: usize = 8;
    let mut has_header = false;
    loop {
        let header_end = offset.checked_add(8)?;
        let header = bytes.get(offset..header_end)?;
        let length = i32::from_be_bytes(header.get(..4)?.try_into().ok()?);
        let length = usize::try_from(length).ok()?;
        let kind = header.get(4..)?;
        if kind == b"IHDR" {
            if length != 13 {
                return None;
            }
            has_header = true;
        }
        match kind {
            b"IDAT" => return Some("image/png"),
            b"acTL" => return None,
            _ if !has_header && kind != b"CgBI" => return None,
            _ => {}
        }
        offset = header_end
            .checked_add(length)?
            .checked_add(4)?
            .min(bytes.len());
        if offset.checked_add(8)? >= bytes.len() {
            return Some("image/png");
        }
    }
}

/// Consume BOM and ID3 envelopes with the detector's 256 re-entry ceiling.
#[cfg(not(target_arch = "wasm32"))]
fn strip_prefixes(mut bytes: &[u8]) -> Option<&[u8]> {
    for depth in 0..=256 {
        let advance = if bytes.starts_with(b"\xef\xbb\xbf") {
            3
        } else if bytes.starts_with(b"ID3") {
            let header = bytes.get(..10)?;
            let size = usize::from(header[9] & 127)
                | (usize::from(header[8]) << 7)
                | (usize::from(header[7]) << 14)
                | (usize::from(header[6]) << 21);
            10 + size
        } else {
            return Some(bytes);
        };
        if depth == 256 {
            return None;
        }
        bytes = bytes.get(advance..)?;
    }
    None
}

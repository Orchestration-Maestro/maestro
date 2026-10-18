//! Owned workbook strings decoded from record payloads.
use super::{
    super::blob::{ParseError, Result, index},
    records::{Payload, Record},
};

/// String representation chosen by the workbook version.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Encoding {
    /// Older workbooks store bytes in the declared code page.
    CodePage(u16),
    /// Newer workbooks store compressed or wide UTF-16 units.
    Unicode,
}

impl Encoding {
    /// Decode `count` characters in this representation.
    pub(super) fn text(self, payload: &mut Payload<'_, '_>, count: usize) -> Result<String> {
        match self {
            Self::CodePage(page) => Ok(code_page(&payload.bytes(count)?, page)),
            Self::Unicode => {
                let wide = payload.u8()? & 1 != 0;
                Ok(String::from_utf16_lossy(&payload.units(count, wide)?))
            }
        }
    }
}

/// Decode declared code-page bytes; an unmapped page keeps each byte as its scalar.
pub(super) fn code_page(bytes: &[u8], page: u16) -> String {
    let encoding: Option<&encoding_rs::Encoding> = codepage::to_encoding_no_replacement(page);
    match encoding {
        Some(encoding) => encoding.decode_without_bom_handling(bytes).0.into_owned(),
        None => bytes.iter().copied().map(char::from).collect(),
    }
}

/// Read a word-counted string, consuming a width flag only for Unicode.
pub(super) fn unicode_string(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<String> {
    let count = usize::from(payload.u16()?);
    encoding.text(payload, count)
}

/// Read a string with a byte-sized character count, as sheet names store it.
pub(super) fn short_string(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<String> {
    let count = usize::from(payload.u8()?);
    encoding.text(payload, count)
}

/// Read a number-format string: byte-counted in older workbooks, word-counted in newer ones.
pub(super) fn format_string(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<String> {
    if encoding == Encoding::Unicode {
        return unicode_string(payload, encoding);
    }
    let count = usize::from(payload.u8()?);
    encoding.text(payload, count)
}

/// Read one shared-table entry, skipping rich-run and extension payloads.
fn rich_extended(payload: &mut Payload<'_, '_>) -> Result<String> {
    let count = usize::from(payload.u16()?);
    let flags = payload.u8()?;
    let runs = if flags & 8 == 0 {
        0
    } else {
        usize::from(payload.u16()?)
    };
    let extension = if flags & 4 == 0 {
        0
    } else {
        usize::try_from(payload.u32()?).map_err(|_| ParseError::Invalid("string"))?
    };
    let text = String::from_utf16_lossy(&payload.units(count, flags & 1 != 0)?);
    payload.skip(runs * 4)?;
    payload.skip(extension)?;
    Ok(text)
}

/// Build the shared-string table once, stopping at its unique count or the payload end.
pub(super) fn shared_strings(record: &Record<'_>) -> Result<Vec<String>> {
    let mut payload = record.payload();
    payload.u32()?;
    let unique = index(payload.u32()?)?;
    let mut strings = Vec::new();
    while strings.len() != unique && payload.remaining() > 0 {
        strings.push(rich_extended(&mut payload)?);
    }
    Ok(strings)
}

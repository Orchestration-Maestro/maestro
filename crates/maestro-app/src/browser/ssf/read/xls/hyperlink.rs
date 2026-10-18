//! Binary hyperlink target selection without path or URL normalization.
use super::{
    super::blob::{ParseError, Result, index},
    records::Payload,
};
use crate::browser::ssf::{CellAddress, CellObject, Hyperlink, encode_cell};
use indexmap::IndexMap;
/// URL moniker identifier in its on-disk byte order.
const URL: [u8; 16] = [
    0xe0, 0xc9, 0xea, 0x79, 0xf9, 0xba, 0xce, 0x11, 0x8c, 0x82, 0, 0xaa, 0, 0x4b, 0xa9, 0x0b,
];
/// File moniker identifier in its on-disk byte order.
const FILE: [u8; 16] = [3, 3, 0, 0, 0, 0, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0x46];
/// URL extension identifier preceding its eight-byte tail.
const URL_EXTRA: [u8; 16] = [
    0x79, 0x58, 0x81, 0xf4, 0x3b, 0x1d, 0x7f, 0x48, 0xaf, 0x2c, 0x82, 0x5d, 0xc4, 0x85, 0x27, 0x63,
];
/// Apply a selected target only to existing cells in the declared rectangle.
pub(super) fn apply(
    payload: &mut Payload<'_, '_>,
    cells: &mut IndexMap<String, CellObject>,
) -> Result<()> {
    let (first, last) = (u32::from(payload.u16()?), u32::from(payload.u16()?));
    let (column, end_column) = (u32::from(payload.u16()?), u32::from(payload.u16()?));
    payload.skip(16)?;
    let target = target(payload)?;
    for r in first..=last {
        for c in column..=end_column {
            #[cfg(test)]
            super::tests::LINK_WORK.with(|work| work.set(work.get() + 1));
            if let Some(cell) = cells.get_mut(&encode_cell(CellAddress { r, c })) {
                cell.l = Some(Hyperlink {
                    target: target.clone(),
                });
            }
        }
    }
    Ok(())
}
/// Decode every flagged field before selecting the first nonempty target.
fn target(payload: &mut Payload<'_, '_>) -> Result<String> {
    if payload.u32()? != 2 {
        return Err(ParseError::Invalid("hyperlink version"));
    }
    let flags = payload.u32()?;
    if flags & 0x10 != 0 {
        counted(payload)?;
    }
    let frame = if flags & 0x80 != 0 {
        counted(payload)?
    } else {
        String::new()
    };
    let moniker = match flags & 0x101 {
        0x101 => counted(payload)?,
        1 => moniker(payload)?,
        _ => String::new(),
    };
    let location = if flags & 8 != 0 {
        counted(payload)?
    } else {
        String::new()
    };
    if flags & 0x20 != 0 {
        payload.skip(16)?;
    }
    if flags & 0x40 != 0 {
        payload.skip(8)?;
    }
    let mut target = if frame.is_empty() { moniker } else { frame };
    if target.is_empty() || !location.is_empty() {
        target.push('#');
        target.push_str(&location);
    }
    if flags & 2 != 0 && target.starts_with('/') && !target.starts_with("//") {
        target.insert_str(0, "file://");
    }
    Ok(target)
}
/// Decode counted raw UTF-16 units, removing encoded NULs but no other characters.
fn counted(payload: &mut Payload<'_, '_>) -> Result<String> {
    let count = index(payload.u32()?)?;
    raw_utf16(payload, count)
}
/// Decode raw units without workbook code pages or continuation-width flags.
fn raw_utf16(payload: &mut Payload<'_, '_>, count: usize) -> Result<String> {
    let bytes = payload.bytes(count.checked_mul(2).ok_or(ParseError::Truncated)?)?;
    Ok(decode_utf16(&bytes))
}
/// Dispatch only the two admitted hyperlink moniker formats.
fn moniker(payload: &mut Payload<'_, '_>) -> Result<String> {
    let id = payload.bytes(16)?;
    if id == URL {
        url(payload)
    } else if id == FILE {
        file(payload)
    } else {
        Err(ParseError::Invalid("hyperlink moniker"))
    }
}
/// Decode a URL and its optional extension tail independently of workbook encoding.
fn url(payload: &mut Payload<'_, '_>) -> Result<String> {
    let count = index(payload.u32()?)?;
    let bytes = payload.bytes(count)?;
    let extra = count > 24 && bytes.get(count - 24..count - 8) == Some(URL_EXTRA.as_slice());
    let text = &bytes[..if extra { count - 24 } else { count }];
    Ok(decode_utf16(text))
}
/// Preserve ANSI and Unicode file-moniker separator rules independently.
fn file(payload: &mut Payload<'_, '_>) -> Result<String> {
    let parent = "../".repeat(usize::from(payload.u16()?));
    let count = index(payload.u32()?)?;
    let bytes = payload.bytes(count)?;
    let ansi = String::from_utf8_lossy(&bytes[..count.saturating_sub(1)]);
    payload.skip(2)?;
    if payload.u16()? != 0xdead {
        return Err(ParseError::Invalid("file moniker magic"));
    }
    if payload.u32()? == 0 {
        return Ok(format!("{parent}{}", ansi.replace('\\', "/")));
    }
    let count = index(payload.u32()?)? / 2;
    if payload.u16()? != 3 {
        return Err(ParseError::Invalid("file moniker kind"));
    }
    Ok(format!("{parent}{}", raw_utf16(payload, count)?))
}

/// Decode the monikers' raw code units without treating a leading BOM specially.
fn decode_utf16(bytes: &[u8]) -> String {
    let units: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(u16::from_le_bytes)
        .collect();
    String::from_utf16_lossy(&units).replace('\0', "")
}

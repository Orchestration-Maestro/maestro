//! Framed workbook records, their payload cursor and fixed-layout fields.
use super::super::blob::{Blob, ParseError, Result};

/// End-of-substream record.
pub(super) const EOF: u16 = 0x000a;

/// One logical record whose continuation payloads stay separate for string decoding.
pub(super) struct Record<'a> {
    /// Record identifier.
    pub(super) kind: u16,
    /// Offset of the record header in the stream.
    pub(super) offset: usize,
    /// Payload pieces in input order.
    pieces: Vec<&'a [u8]>,
}

impl<'a> Record<'a> {
    /// Cursor over the whole logical payload.
    pub(super) fn payload(&self) -> Payload<'_, 'a> {
        let mut pieces = self.pieces.iter();
        let current = Blob::new(pieces.next().copied().unwrap_or_default());
        let later_bytes = pieces
            .clone()
            .map(|piece| {
                #[cfg(test)]
                super::tests::PAYLOAD_WORK.with(|work| work.set(work.get() + 1));
                piece.len()
            })
            .sum();
        Payload {
            current,
            pieces,
            later_bytes,
        }
    }
}

/// One linear pass over a workbook stream.
pub(super) struct Records<'a> {
    /// Remaining stream bytes.
    blob: Blob<'a>,
    /// Stream length, locating each record.
    length: usize,
    /// Previous record identifier.
    previous: u16,
}

impl<'a> Records<'a> {
    /// Start at the beginning of a workbook stream.
    pub(super) fn new(stream: &'a [u8]) -> Self {
        Self {
            blob: Blob::new(stream),
            length: stream.len(),
            previous: 0,
        }
    }
    /// Read the next record with its following continuation records.
    pub(super) fn next(&mut self) -> Result<Option<Record<'a>>> {
        if self.blob.rest.len() < 2 {
            return Ok(None);
        }
        let offset = self.length - self.blob.rest.len();
        let kind = self.blob.u16()?;
        if kind == 0 && self.previous == EOF {
            return Ok(None);
        }
        self.previous = kind;
        let size = if self.blob.rest.is_empty() {
            0
        } else {
            self.blob.u16()?
        };
        let mut pieces = vec![self.blob.take(usize::from(size))?];
        while let Some(skip) = continuation(self.blob.rest) {
            self.blob.u16()?;
            let size = usize::from(self.blob.u16()?);
            pieces.push(self.blob.take(size)?.get(skip..).unwrap_or_default());
        }
        Ok(Some(Record {
            kind,
            offset,
            pieces,
        }))
    }
}

/// Header bytes skipped by a continuation record starting `rest`, if it is one.
fn continuation(rest: &[u8]) -> Option<usize> {
    match u16::from_le_bytes([*rest.first()?, *rest.get(1)?]) {
        0x003c | 0x043c => Some(0),
        0x0812 => Some(4),
        0x0875 | 0x087f => Some(12),
        _ => None,
    }
}

/// Cursor across a record's payload pieces; only character data observes their boundaries.
pub(super) struct Payload<'r, 'a> {
    /// Current piece.
    current: Blob<'a>,
    /// Later pieces.
    pieces: std::slice::Iter<'r, &'a [u8]>,
    /// Unread bytes in the later pieces, excluding the current piece.
    later_bytes: usize,
}

impl Payload<'_, '_> {
    /// Move to the next piece.
    fn advance(&mut self) -> Result<()> {
        let next = self.pieces.next().ok_or(ParseError::Truncated)?;
        #[cfg(test)]
        super::tests::PAYLOAD_WORK.with(|work| work.set(work.get() + 1));
        self.later_bytes -= next.len();
        self.current = Blob::new(next);
        Ok(())
    }
    /// Unread bytes across all pieces.
    pub(super) fn remaining(&self) -> usize {
        #[cfg(test)]
        super::tests::PAYLOAD_WORK.with(|work| work.set(work.get() + 1));
        self.current.rest.len() + self.later_bytes
    }
    /// Read one byte, crossing a piece boundary transparently.
    pub(super) fn u8(&mut self) -> Result<u8> {
        while self.current.rest.is_empty() {
            self.advance()?;
        }
        self.current.u8()
    }
    /// Read one little-endian word.
    pub(super) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes([self.u8()?, self.u8()?]))
    }
    /// Read one little-endian double word.
    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes([
            self.u8()?,
            self.u8()?,
            self.u8()?,
            self.u8()?,
        ]))
    }
    /// Read one binary64 number.
    pub(super) fn f64(&mut self) -> Result<f64> {
        let mut bytes = [0; 8];
        for byte in &mut bytes {
            *byte = self.u8()?;
        }
        Ok(f64::from_le_bytes(bytes))
    }
    /// Read raw bytes, crossing piece boundaries transparently.
    pub(super) fn bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        (0..count).map(|_| self.u8()).collect()
    }
    /// Skip bytes, crossing piece boundaries transparently.
    pub(super) fn skip(&mut self, count: usize) -> Result<()> {
        self.bytes(count).map(drop)
    }
    /// Read UTF-16 units; a character starting a new piece first reads that piece's width flag.
    pub(super) fn units(&mut self, count: usize, mut wide: bool) -> Result<Vec<u16>> {
        let mut units = Vec::with_capacity(count);
        for _ in 0..count {
            if self.current.rest.is_empty() {
                self.advance()?;
                wide = self.current.u8()? & 1 != 0;
            }
            units.push(if wide {
                self.current.u16()?
            } else {
                u16::from(self.current.u8()?)
            });
        }
        Ok(units)
    }
}

/// Cell position and extended-format index common to cell records.
#[derive(Clone, Copy)]
pub(super) struct CellHeader {
    /// Zero-based row.
    pub(super) r: u32,
    /// Zero-based column.
    pub(super) c: u32,
    /// Extended-format index.
    pub(super) ixfe: u16,
}

impl CellHeader {
    /// Read a row, column and format index.
    pub(super) fn parse(payload: &mut Payload<'_, '_>) -> Result<Self> {
        Ok(Self {
            r: u32::from(payload.u16()?),
            c: u32::from(payload.u16()?),
            ixfe: payload.u16()?,
        })
    }
}

/// Decode a compressed number: integer or high-word float, optionally scaled by 100.
pub(super) fn rk_number(payload: &mut Payload<'_, '_>) -> Result<f64> {
    let raw = payload.u32()?;
    let value = if raw & 2 == 0 {
        f64::from_bits(u64::from(raw & !3) << 32)
    } else {
        f64::from(i32::from_le_bytes(raw.to_le_bytes()) >> 2)
    };
    Ok(if raw & 1 == 0 { value } else { value / 100.0 })
}

/// Cached formula result.
pub(super) enum FormulaValue {
    /// Number result.
    Number(f64),
    /// Text supplied by the following string record.
    PendingString,
    /// Boolean result.
    Boolean(bool),
    /// Error code result.
    Error(u8),
    /// Empty text result.
    Empty,
    /// Unknown result type, which produces no cell.
    Unknown,
}

impl FormulaValue {
    /// Classify the eight cached-result bytes.
    pub(super) fn parse(payload: &mut Payload<'_, '_>) -> Result<Self> {
        let bytes = payload.bytes(8)?;
        if bytes[6..] != [0xff, 0xff] {
            return Blob::new(&bytes).f64().map(Self::Number);
        }
        Ok(match bytes[0] {
            0 => Self::PendingString,
            1 => Self::Boolean(bytes[2] == 1),
            2 => Self::Error(bytes[2]),
            3 => Self::Empty,
            _ => Self::Unknown,
        })
    }
}

/// Inclusive-start, exclusive-end rectangle as zero-based (row, column) pairs.
pub(super) type Bounds = ((u32, u32), (u32, u32));

/// Read declared sheet bounds; rows widen to double words in the newer layout.
pub(super) fn dimensions(payload: &mut Payload<'_, '_>, wide: bool) -> Result<Bounds> {
    let mut row = || -> Result<u32> {
        if wide {
            payload.u32()
        } else {
            payload.u16().map(u32::from)
        }
    };
    let (r, last_r) = (row()?, row()?);
    let (c, last_c) = (u32::from(payload.u16()?), u32::from(payload.u16()?));
    Ok(((r, c), (last_r, last_c)))
}

/// Read inclusive merged-cell rectangles.
pub(super) fn merge_cells(payload: &mut Payload<'_, '_>) -> Result<Vec<Bounds>> {
    (0..payload.u16()?)
        .map(|_| {
            let (r, last_r) = (u32::from(payload.u16()?), u32::from(payload.u16()?));
            let (c, last_c) = (u32::from(payload.u16()?), u32::from(payload.u16()?));
            Ok(((r, c), (last_r, last_c)))
        })
        .collect()
}

/// Compressed numbers sharing one row, as cell headers with their values.
pub(super) type MulRk = Vec<(CellHeader, f64)>;

/// Read a multiple compressed-number record.
pub(super) fn mul_rk(payload: &mut Payload<'_, '_>) -> Result<MulRk> {
    let body = payload.remaining().saturating_sub(6);
    if payload.remaining() < 6 || !body.is_multiple_of(6) {
        return Err(ParseError::Invalid("multiple number record"));
    }
    let (r, c) = (u32::from(payload.u16()?), u32::from(payload.u16()?));
    let values = (c..)
        .take(body / 6)
        .map(|column| {
            let ixfe = payload.u16()?;
            Ok((CellHeader { r, c: column, ixfe }, rk_number(payload)?))
        })
        .collect::<Result<MulRk>>()?;
    let last = u32::from(payload.u16()?);
    if u32::try_from(values.len()).ok().map(|n| c + n) != Some(last + 1) {
        return Err(ParseError::Invalid("multiple number record"));
    }
    Ok(values)
}

/// Read a substream's version and type; long records with unknown versions are invalid.
pub(super) fn bof(payload: &mut Payload<'_, '_>) -> Result<(u16, u16)> {
    let version = payload.u16()?;
    let kind = if payload.remaining() >= 2 {
        payload.u16()?
    } else {
        0
    };
    let known = matches!(
        version,
        0x0600 | 0x0500 | 0x0400 | 0x0300 | 0x0200 | 0x0002 | 0x0007
    );
    if !known && payload.remaining() > 4 {
        return Err(ParseError::Invalid("substream version"));
    }
    Ok((version, kind))
}

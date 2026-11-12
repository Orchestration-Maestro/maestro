//! Checked little-endian reads shared by compound-file and record decoding.
use std::fmt;

/// Why a legacy workbook's bytes could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParseError {
    /// An offset or length extends beyond the supplied bytes.
    Truncated,
    /// A header, record or reference is malformed.
    Invalid(&'static str),
    /// The workbook is encrypted.
    Encrypted,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated data"),
            Self::Invalid(what) => write!(f, "invalid {what}"),
            Self::Encrypted => f.write_str("file is password-protected"),
        }
    }
}

/// Result of a private byte operation.
pub(super) type Result<T> = std::result::Result<T, ParseError>;

/// Borrowed input cursor that never outlives one synchronous read.
#[derive(Clone, Copy)]
pub(super) struct Blob<'a> {
    /// Unconsumed bytes.
    pub(super) rest: &'a [u8],
}

impl<'a> Blob<'a> {
    /// Borrow bytes from their start.
    pub(super) fn new(rest: &'a [u8]) -> Self {
        Self { rest }
    }
    /// Borrow bytes from a checked offset.
    pub(super) fn at(data: &'a [u8], offset: usize) -> Result<Self> {
        data.get(offset..)
            .map(Self::new)
            .ok_or(ParseError::Truncated)
    }
    /// Consume a complete byte range.
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let (head, tail) = self
            .rest
            .split_at_checked(count)
            .ok_or(ParseError::Truncated)?;
        self.rest = tail;
        Ok(head)
    }
    /// Consume a fixed-width array.
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| ParseError::Truncated)
    }
    /// Read one byte.
    pub(super) fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    /// Read one little-endian word.
    pub(super) fn u16(&mut self) -> Result<u16> {
        self.array().map(u16::from_le_bytes)
    }
    /// Read one little-endian double word.
    pub(super) fn u32(&mut self) -> Result<u32> {
        self.array().map(u32::from_le_bytes)
    }
    /// Read one binary64 number without text conversion.
    pub(super) fn f64(&mut self) -> Result<f64> {
        self.array().map(f64::from_le_bytes)
    }
}

/// Convert an on-disk count or offset before indexing memory.
pub(super) fn index(value: u32) -> Result<usize> {
    usize::try_from(value).map_err(|_| ParseError::Invalid("offset"))
}

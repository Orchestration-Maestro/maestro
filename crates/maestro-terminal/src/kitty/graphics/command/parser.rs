//! Incremental control parsing and payload decoding.
use super::{Command, Control, ParseError, Quiet};
use base64::{
    Engine as _, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

/// Parser phase, including malformed controls that must be skipped.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// Collecting a one-byte key.
    Key,
    /// Ignoring an oversized key until its equals sign.
    IgnoreKey,
    /// Collecting a scalar value.
    Value,
    /// Ignoring a malformed control value.
    IgnoreValue,
    /// Collecting encoded payload bytes.
    Data,
}

/// A bounded one-byte-key control dictionary.
pub(super) struct Keys([Option<u32>; 256]);
impl Keys {
    /// Read a field with its wire default.
    pub(super) fn get(&self, key: u8, default: u32) -> u32 {
        self.0[usize::from(key)].unwrap_or(default)
    }
    /// Read a field without replacing absence.
    pub(super) fn optional(&self, key: u8) -> Option<u32> {
        self.0[usize::from(key)]
    }
    /// Read a required field.
    pub(super) fn required(&self, key: u8) -> Result<u32, ParseError> {
        self.0[usize::from(key)].ok_or(ParseError::InvalidFormat)
    }
    /// Read a signed field's original bits.
    pub(super) fn signed(&self, key: u8) -> i32 {
        i32::from_ne_bytes(self.get(key, 0).to_ne_bytes())
    }
}

/// Incremental parser for the bytes after an image command introducer.
pub struct Parser {
    /// Current control or payload phase.
    state: State,
    /// Completed control fields.
    keys: Keys,
    /// Incomplete key or scalar, at most eleven bytes.
    pending: Vec<u8>,
    /// Key associated with the incomplete scalar.
    key: u8,
    /// Encoded payload collected so far.
    data: Vec<u8>,
    /// Maximum encoded payload size.
    max_bytes: usize,
}
impl Parser {
    /// Start one command with an encoded-payload byte budget.
    #[must_use]
    pub fn init(max_bytes: usize) -> Self {
        Self {
            state: State::Key,
            keys: Keys([None; 256]),
            pending: Vec::new(),
            key: 0,
            data: Vec::new(),
            max_bytes,
        }
    }
    /// Parse one command with a 1 MiB encoded-payload budget.
    ///
    /// # Errors
    /// Returns the control, scalar, payload or budget error encountered.
    pub fn parse_string(data: &[u8]) -> Result<Command, ParseError> {
        let mut parser = Self::init(1024 * 1024);
        parser.feed_slice(data)?;
        parser.complete()
    }
    /// Feed one byte, retaining partial controls and payload.
    ///
    /// # Errors
    /// Returns a scalar error or an exhausted payload budget.
    pub fn feed(&mut self, byte: u8) -> Result<(), ParseError> {
        match (self.state, byte) {
            (State::Key, b'=') => {
                if self.pending.len() == 1 {
                    self.key = self.pending[0];
                    self.state = State::Value;
                } else {
                    self.state = State::IgnoreValue;
                }
                self.pending.clear();
            }
            (State::Key | State::IgnoreValue, b';') => self.state = State::Data,
            (State::Key, _) => self.accumulate(byte, State::IgnoreKey),
            (State::IgnoreKey, b'=') => self.state = State::IgnoreValue,
            (State::Value, b',') => self.finish_value(State::Key)?,
            (State::Value, b';') => self.finish_value(State::Data)?,
            (State::Value, _) => self.accumulate(byte, State::IgnoreValue),
            (State::IgnoreValue, b',') => self.state = State::IgnoreKey,
            (State::IgnoreKey | State::IgnoreValue, _) => {}
            (State::Data, _) => self.append(&[byte])?,
        }
        Ok(())
    }
    /// Feed a slice, appending the payload remainder in one operation.
    ///
    /// # Errors
    /// Returns the same errors as byte feeding.
    pub fn feed_slice(&mut self, bytes: &[u8]) -> Result<(), ParseError> {
        for (index, &byte) in bytes.iter().enumerate() {
            if self.state == State::Data {
                return self.append(&bytes[index..]);
            }
            self.feed(byte)?;
        }
        Ok(())
    }
    /// Finish control selection and decode the owned payload.
    ///
    /// # Errors
    /// Returns an incomplete/invalid control or malformed payload error.
    pub fn complete(mut self) -> Result<Command, ParseError> {
        match self.state {
            State::Key | State::IgnoreKey => return Err(ParseError::InvalidFormat),
            State::Value => self.finish_value(State::Data)?,
            State::IgnoreValue | State::Data => {}
        }
        let control = Control::parse(&self.keys)?;
        let quiet = match self.keys.get(b'q', 0) {
            0 => Quiet::No,
            1 => Quiet::Ok,
            2 => Quiet::Failures,
            _ => return Err(ParseError::InvalidFormat),
        };
        Ok(Command {
            control,
            quiet,
            data: decode_payload(self.data)?,
        })
    }
    /// Collect a bounded key or value and enter its ignore state on overflow.
    fn accumulate(&mut self, byte: u8, overflow: State) {
        if self.pending.len() == 11 {
            self.pending.clear();
            self.state = overflow;
        } else {
            self.pending.push(byte);
        }
    }
    /// Classify and store a scalar before entering the next phase.
    fn finish_value(&mut self, next: State) -> Result<(), ParseError> {
        self.state = next;
        let value = match self.pending.as_slice() {
            [byte] if !byte.is_ascii_digit() => u32::from(*byte),
            bytes => parse_scalar(bytes, matches!(self.key, b'z' | b'H' | b'V'))?,
        };
        self.keys.0[usize::from(self.key)] = Some(value);
        self.pending.clear();
        Ok(())
    }
    /// Append without exceeding the encoded byte budget.
    fn append(&mut self, bytes: &[u8]) -> Result<(), ParseError> {
        if bytes.len() > self.max_bytes.saturating_sub(self.data.len()) {
            return Err(ParseError::OutOfMemory);
        }
        self.data
            .try_reserve(bytes.len())
            .map_err(|_| ParseError::OutOfMemory)?;
        self.data.extend_from_slice(bytes);
        Ok(())
    }
}

/// Parse decimal bytes with optional sign and interior underscores.
fn parse_scalar(bytes: &[u8], signed: bool) -> Result<u32, ParseError> {
    let (negative, digits) = match bytes {
        [b'-', rest @ ..] => (true, rest),
        [b'+', rest @ ..] => (false, rest),
        _ => (false, bytes),
    };
    if digits.is_empty() || digits.first() == Some(&b'_') || digits.last() == Some(&b'_') {
        return Err(ParseError::InvalidCharacter);
    }
    let mut value = 0_i64;
    for &digit in digits {
        if digit == b'_' {
            continue;
        }
        if !digit.is_ascii_digit() {
            return Err(ParseError::InvalidCharacter);
        }
        let digit = i64::from(digit - b'0');
        value = value * 10 + if negative { -digit } else { digit };
        if signed {
            if i32::try_from(value).is_err() {
                return Err(ParseError::Overflow);
            }
        } else if u32::try_from(value).is_err() {
            return Err(ParseError::Overflow);
        }
    }
    if signed {
        Ok(u32::from_ne_bytes(
            i32::try_from(value)
                .map_err(|_| ParseError::Overflow)?
                .to_ne_bytes(),
        ))
    } else {
        u32::try_from(value).map_err(|_| ParseError::Overflow)
    }
}

/// Apply the shipped standard-alphabet whitespace and padding boundary.
fn decode_payload(mut bytes: Vec<u8>) -> Result<Vec<u8>, ParseError> {
    bytes.retain(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 12));
    if bytes.contains(&b'=') && !bytes.len().is_multiple_of(4) {
        return Err(ParseError::InvalidData);
    }
    let decoder = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    decoder.decode(bytes).map_err(|_| ParseError::InvalidData)
}

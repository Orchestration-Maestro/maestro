//! Lenient UTF-8 decoding of response bytes.

use std::borrow::Cow;

/// The bytes of a byte-order mark.
const BYTE_ORDER_MARK: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Decode `bytes` as a default text decoder does: one leading byte-order mark is dropped and
/// every maximal invalid sequence becomes one U+FFFD.
pub(crate) fn decode_utf8(bytes: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(bytes.strip_prefix(&BYTE_ORDER_MARK).unwrap_or(bytes))
}

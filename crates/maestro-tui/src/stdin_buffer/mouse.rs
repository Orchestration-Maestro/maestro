//! Borrowed SGR report fields without caller-specific numeric admission.

/// Raw fields of one complete SGR mouse report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SgrMouseFields<'a> {
    /// Encoded button and modifier field.
    pub cb: &'a [u8],
    /// One-based column text.
    pub column: &'a [u8],
    /// One-based row text.
    pub row: &'a [u8],
    /// Any additional fields, retained without interpretation.
    pub extra: Option<&'a [u8]>,
    /// Whether the report ends with the release terminator.
    pub release: bool,
}

/// Splits a complete SGR report without imposing either consumer's numeric domain.
#[must_use]
pub fn scan_sgr_mouse_fields(data: &[u8]) -> Option<SgrMouseFields<'_>> {
    let body = data.strip_prefix(b"\x1b[<")?;
    let (body, release) = if let Some(body) = body.strip_suffix(b"M") {
        (body, false)
    } else {
        (body.strip_suffix(b"m")?, true)
    };
    let mut fields = body.splitn(4, |byte| *byte == b';');
    Some(SgrMouseFields {
        cb: fields.next()?,
        column: fields.next()?,
        row: fields.next()?,
        extra: fields.next(),
        release,
    })
}

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

/// Admitted host mouse fields, independent of any wire representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostMouse {
    /// Button and modifier byte.
    pub cb: u8,
    /// Zero-based column.
    pub column: u16,
    /// Zero-based row.
    pub row: u16,
    /// Release terminator rather than press or motion.
    pub release: bool,
}

/// Admit a complete SGR or six-byte default mouse frame in the host numeric domain.
#[must_use]
pub fn parse_host_mouse(data: &[u8]) -> Option<HostMouse> {
    let (cb, column, row, release) = if let [27, b'[', b'M', cb, column, row] = *data {
        (
            cb.checked_sub(32)?,
            u16::from(column).checked_sub(33)?,
            u16::from(row).checked_sub(33)?,
            false,
        )
    } else {
        let fields = scan_sgr_mouse_fields(data)?;
        (
            std::str::from_utf8(fields.cb).ok()?.parse().ok()?,
            std::str::from_utf8(fields.column)
                .ok()?
                .parse::<u16>()
                .ok()?
                .checked_sub(1)?,
            std::str::from_utf8(fields.row)
                .ok()?
                .parse::<u16>()
                .ok()?
                .checked_sub(1)?,
            fields.release,
        )
    };
    let button = (cb & 3) | ((cb & 0xc0) >> 4);
    let dragging = cb & 32 != 0;
    let admitted = if dragging {
        matches!(button, 0..=5 | 8 | 9)
    } else {
        button <= 7
    };
    admitted.then_some(HostMouse {
        cb,
        column,
        row,
        release,
    })
}

//! Byte mappings for terminal character sets.

/// The available terminal character-set slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slots {
    /// Primary character set.
    G0,
    /// First alternate character set.
    G1,
    /// Second alternate character set.
    G2,
    /// Third alternate character set.
    G3,
}

/// The active left and right character-set slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveSlot {
    /// Left character set.
    Gl,
    /// Right character set.
    Gr,
}

/// Supported terminal character sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    /// Unicode input without a byte mapping.
    Utf8,
    /// Identity byte mapping.
    Ascii,
    /// British mapping with the pound sign.
    British,
    /// DEC special graphics mapping.
    DecSpecial,
}

/// Returns the byte mapping, or `None` for Unicode input.
#[must_use]
pub const fn table(set: Charset) -> Option<&'static [u16; 256]> {
    match set {
        Charset::Utf8 => None,
        Charset::Ascii => Some(&ASCII),
        Charset::British => Some(&BRITISH),
        Charset::DecSpecial => Some(&DEC_SPECIAL),
    }
}

/// Identity byte mapping.
const ASCII: [u16; 256] = init_table();

/// British byte mapping.
const BRITISH: [u16; 256] = {
    let mut table = ASCII;
    table[0x23] = 0x00a3;
    table
};

/// DEC special graphics byte mapping.
const DEC_SPECIAL: [u16; 256] = {
    let mut table = ASCII;
    table[0x60] = 0x25c6;
    table[0x61] = 0x2592;
    table[0x62] = 0x2409;
    table[0x63] = 0x240c;
    table[0x64] = 0x240d;
    table[0x65] = 0x240a;
    table[0x66] = 0x00b0;
    table[0x67] = 0x00b1;
    table[0x68] = 0x2424;
    table[0x69] = 0x240b;
    table[0x6a] = 0x2518;
    table[0x6b] = 0x2510;
    table[0x6c] = 0x250c;
    table[0x6d] = 0x2514;
    table[0x6e] = 0x253c;
    table[0x6f] = 0x23ba;
    table[0x70] = 0x23bb;
    table[0x71] = 0x2500;
    table[0x72] = 0x23bc;
    table[0x73] = 0x23bd;
    table[0x74] = 0x251c;
    table[0x75] = 0x2524;
    table[0x76] = 0x2534;
    table[0x77] = 0x252c;
    table[0x78] = 0x2502;
    table[0x79] = 0x2264;
    table[0x7a] = 0x2265;
    table[0x7b] = 0x03c0;
    table[0x7c] = 0x2260;
    table[0x7d] = 0x00a3;
    table[0x7e] = 0x00b7;
    table
};

/// Builds the identity mapping at compile time.
const fn init_table() -> [u16; 256] {
    let mut table = [0; 256];
    let mut byte = 0u16;
    while byte < 256 {
        table[byte as usize] = byte;
        byte += 1;
    }
    table
}

//! Named ANSI colours and the default indexed palette.

use super::{Palette, Rgb};

/// A palette index, including indices with no built-in name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Name(pub u8);

/// An unnamed palette index has no built-in default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoDefaultValue;
impl std::fmt::Display for NoDefaultValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NoDefaultValue")
    }
}
impl std::error::Error for NoDefaultValue {}

impl Name {
    /// The black ANSI index.
    pub const BLACK: Self = Self(0);
    /// The red ANSI index.
    pub const RED: Self = Self(1);
    /// The green ANSI index.
    pub const GREEN: Self = Self(2);
    /// The yellow ANSI index.
    pub const YELLOW: Self = Self(3);
    /// The blue ANSI index.
    pub const BLUE: Self = Self(4);
    /// The magenta ANSI index.
    pub const MAGENTA: Self = Self(5);
    /// The cyan ANSI index.
    pub const CYAN: Self = Self(6);
    /// The white ANSI index.
    pub const WHITE: Self = Self(7);
    /// The bright black ANSI index.
    pub const BRIGHT_BLACK: Self = Self(8);
    /// The bright red ANSI index.
    pub const BRIGHT_RED: Self = Self(9);
    /// The bright green ANSI index.
    pub const BRIGHT_GREEN: Self = Self(10);
    /// The bright yellow ANSI index.
    pub const BRIGHT_YELLOW: Self = Self(11);
    /// The bright blue ANSI index.
    pub const BRIGHT_BLUE: Self = Self(12);
    /// The bright magenta ANSI index.
    pub const BRIGHT_MAGENTA: Self = Self(13);
    /// The bright cyan ANSI index.
    pub const BRIGHT_CYAN: Self = Self(14);
    /// The bright white ANSI index.
    pub const BRIGHT_WHITE: Self = Self(15);
    /// Return the built-in colour for one of the sixteen named indices.
    ///
    /// # Errors
    /// Returns `NoDefaultValue` for indices 16 through 255.
    pub fn default(self) -> Result<Rgb, NoDefaultValue> {
        BASE.get(usize::from(self.0)).copied().ok_or(NoDefaultValue)
    }
}

/// The sixteen base ANSI colours.
const BASE: [Rgb; 16] = [
    Rgb {
        r: 0x1D,
        g: 0x1F,
        b: 0x21,
    },
    Rgb {
        r: 0xCC,
        g: 0x66,
        b: 0x66,
    },
    Rgb {
        r: 0xB5,
        g: 0xBD,
        b: 0x68,
    },
    Rgb {
        r: 0xF0,
        g: 0xC6,
        b: 0x74,
    },
    Rgb {
        r: 0x81,
        g: 0xA2,
        b: 0xBE,
    },
    Rgb {
        r: 0xB2,
        g: 0x94,
        b: 0xBB,
    },
    Rgb {
        r: 0x8A,
        g: 0xBE,
        b: 0xB7,
    },
    Rgb {
        r: 0xC5,
        g: 0xC8,
        b: 0xC6,
    },
    Rgb {
        r: 0x66,
        g: 0x66,
        b: 0x66,
    },
    Rgb {
        r: 0xD5,
        g: 0x4E,
        b: 0x53,
    },
    Rgb {
        r: 0xB9,
        g: 0xCA,
        b: 0x4A,
    },
    Rgb {
        r: 0xE7,
        g: 0xC5,
        b: 0x47,
    },
    Rgb {
        r: 0x7A,
        g: 0xA6,
        b: 0xDA,
    },
    Rgb {
        r: 0xC3,
        g: 0x97,
        b: 0xD8,
    },
    Rgb {
        r: 0x70,
        g: 0xC0,
        b: 0xB1,
    },
    Rgb {
        r: 0xEA,
        g: 0xEA,
        b: 0xEA,
    },
];

/// The default 256-colour terminal palette.
pub const DEFAULT: Palette = default_palette();

/// Assemble the base colours, six-level cube and gray ramp.
const fn default_palette() -> Palette {
    let mut result = [Rgb { r: 0, g: 0, b: 0 }; 256];
    let mut i = 0;
    while i < 16 {
        result[i] = BASE[i];
        i += 1;
    }
    let levels = [0, 95, 135, 175, 215, 255];
    let mut r = 0;
    while r < 6 {
        let mut g = 0;
        while g < 6 {
            let mut b = 0;
            while b < 6 {
                result[i] = Rgb {
                    r: levels[r],
                    g: levels[g],
                    b: levels[b],
                };
                i += 1;
                b += 1;
            }
            g += 1;
        }
        r += 1;
    }
    let mut value = 8;
    while i < 256 {
        result[i] = Rgb {
            r: value,
            g: value,
            b: value,
        };
        i += 1;
        value += 10;
    }
    result
}

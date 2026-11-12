//! Retained palette defaults and per-index overrides.

use super::{DEFAULT, Palette, Rgb};

/// The positional set of overridden palette entries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaletteMask([u64; 4]);

impl PaletteMask {
    /// Read one override bit.
    #[must_use]
    pub const fn is_set(self, index: u8) -> bool {
        self.0[index as usize / 64] & (1 << (index % 64)) != 0
    }
    /// Mark one index as overridden.
    pub fn set(&mut self, index: u8) {
        self.0[usize::from(index) / 64] |= 1 << (index % 64);
    }
    /// Clear one override bit.
    pub fn unset(&mut self, index: u8) {
        self.0[usize::from(index) / 64] &= !(1 << (index % 64));
    }
}

/// Current colours with independently replaceable defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicPalette {
    /// Current colours, including overrides.
    pub current: Palette,
    /// Most recently supplied defaults.
    pub original: Palette,
    /// Indices explicitly overridden by the caller.
    pub mask: PaletteMask,
}

impl DynamicPalette {
    /// Built-in defaults with no overridden entries.
    pub const DEFAULT: Self = Self::init(DEFAULT);
    /// Retain the supplied defaults without overrides.
    #[must_use]
    pub const fn init(def: Palette) -> Self {
        Self {
            current: def,
            original: def,
            mask: PaletteMask([0; 4]),
        }
    }
    /// Override an individual colour.
    pub fn set(&mut self, idx: u8, color: Rgb) {
        self.current[usize::from(idx)] = color;
        self.mask.set(idx);
    }
    /// Restore the current default of one entry.
    pub fn reset(&mut self, idx: u8) {
        self.current[usize::from(idx)] = self.original[usize::from(idx)];
        self.mask.unset(idx);
    }
    /// Restore all current defaults and clear every override.
    pub fn reset_all(&mut self) {
        *self = Self::init(self.original);
    }
    /// Replace defaults while retaining explicitly overridden entries.
    pub fn change_default(&mut self, def: Palette) {
        self.original = def;
        for index in 0..=u8::MAX {
            if !self.mask.is_set(index) {
                self.current[usize::from(index)] = def[usize::from(index)];
            }
        }
    }
}
impl Default for DynamicPalette {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Optional default and explicit override of one dynamic colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DynamicRgb {
    /// Explicit override, when present.
    pub r#override: Option<Rgb>,
    /// Default supplied by the caller.
    pub default: Option<Rgb>,
}
impl DynamicRgb {
    /// No default or override.
    pub const UNSET: Self = Self {
        r#override: None,
        default: None,
    };
    /// Keep the supplied default with no override.
    #[must_use]
    pub const fn init(def: Rgb) -> Self {
        Self {
            r#override: None,
            default: Some(def),
        }
    }
    /// Select the override before the default.
    #[must_use]
    pub const fn get(&self) -> Option<Rgb> {
        match self.r#override {
            Some(color) => Some(color),
            None => self.default,
        }
    }
    /// Replace the explicit override.
    pub fn set(&mut self, color: Rgb) {
        self.r#override = Some(color);
    }
    /// Copy the current default into the override.
    pub fn reset(&mut self) {
        self.r#override = self.default;
    }
}

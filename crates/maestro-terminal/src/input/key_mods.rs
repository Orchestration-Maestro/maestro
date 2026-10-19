//! Modifier keys, lock state and independently observed modifier sides.

/// A standard modifier key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mod {
    /// Shift.
    Shift,
    /// Control.
    Ctrl,
    /// Alt or option.
    Alt,
    /// Super or command.
    Super,
}

/// The observed side of a modifier, independent of whether it is pressed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    /// Left-hand modifier, also the default when no side is supplied.
    #[default]
    Left,
    /// Right-hand modifier.
    Right,
}

/// Modifier mask: keys in bits 0–3, locks in 4–5 and right sides in 6–9.
///
/// A side bit does not imply that the modifier is pressed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods(u16);

impl Mods {
    /// Shift is pressed.
    pub const SHIFT: Self = Self(1);
    /// Control is pressed.
    pub const CTRL: Self = Self(2);
    /// Alt is pressed.
    pub const ALT: Self = Self(4);
    /// Super is pressed.
    pub const SUPER: Self = Self(8);
    /// Caps lock is enabled.
    pub const CAPS_LOCK: Self = Self(16);
    /// Num lock is enabled.
    pub const NUM_LOCK: Self = Self(32);

    /// Preserve a supplied modifier mask, including side and reserved bits.
    #[must_use]
    pub const fn from_int(bits: u16) -> Self {
        Self(bits)
    }

    /// Return the full mask, including lock and side bits.
    #[must_use]
    pub const fn int(self) -> u16 {
        self.0
    }

    /// Whether no bits are set, including side and lock bits.
    #[must_use]
    pub const fn empty(self) -> bool {
        self.0 == 0
    }

    /// Whether every bit in the supplied mask is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Combine modifier observations.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Remove the supplied bits, leaving all other observations unchanged.
    #[must_use]
    pub const fn unset(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Retain only standard modifier keys, excluding locks and sides.
    #[must_use]
    pub const fn binding(self) -> Self {
        Self(self.0 & 0x0f)
    }

    /// Remove caps-lock and num-lock observations only.
    #[must_use]
    pub const fn without_locks(self) -> Self {
        Self(self.0 & !0x30)
    }

    /// Return the observed side without testing whether the key is pressed.
    #[must_use]
    pub const fn side(self, modifier: Mod) -> Side {
        if self.0 & (1 << (6 + modifier as u8)) == 0 {
            Side::Left
        } else {
            Side::Right
        }
    }

    /// Change a side observation without changing pressed keys or locks.
    #[must_use]
    pub const fn with_side(self, modifier: Mod, side: Side) -> Self {
        let bit = 1 << (6 + modifier as u8);
        Self(match side {
            Side::Left => self.0 & !bit,
            Side::Right => self.0 | bit,
        })
    }
}

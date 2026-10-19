//! Keyboard protocol flags with an eight-entry, allocation-free history.

/// Enabled keyboard protocol enhancements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags(u8);

impl Flags {
    /// Disable all enhancements.
    pub const DISABLED: Self = Self(0);
    /// Disambiguate escape codes.
    pub const DISAMBIGUATE: Self = Self(1);
    /// Report key press, repeat and release events.
    pub const REPORT_EVENTS: Self = Self(2);
    /// Report alternate key codes.
    pub const REPORT_ALTERNATES: Self = Self(4);
    /// Report all keys as escape sequences.
    pub const REPORT_ALL: Self = Self(8);
    /// Report associated text.
    pub const REPORT_ASSOCIATED: Self = Self(16);
    /// Enable all five enhancements.
    pub const TRUE: Self = Self(31);

    /// Construct flags only when all bits belong to the five-bit protocol domain.
    #[must_use]
    pub fn from_int(value: u16) -> Option<Self> {
        u8::try_from(value)
            .ok()
            .filter(|&bits| bits <= 31)
            .map(Self)
    }

    /// Return the protocol bit representation.
    #[must_use]
    pub fn int(self) -> u8 {
        self.0
    }
}

/// Operation applied to the current keyboard protocol flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetMode {
    /// Replace the current flags.
    Set,
    /// Enable the supplied flags.
    Or,
    /// Disable the supplied flags.
    Not,
}

/// Keyboard protocol history, evicting the oldest entry when full.
#[derive(Clone, Debug, Default)]
pub struct FlagStack {
    /// Retained flag values, including cleared entries.
    flags: [Flags; 8],
    /// Index of the current value.
    index: usize,
}

impl FlagStack {
    /// Return the current keyboard protocol flags.
    #[must_use]
    pub fn current(&self) -> Flags {
        self.flags[self.index]
    }

    /// Apply an operation to the current flags without changing history.
    pub fn set(&mut self, mode: SetMode, flags: Flags) {
        let current = &mut self.flags[self.index];
        *current = match mode {
            SetMode::Set => flags,
            SetMode::Or => Flags(current.0 | flags.0),
            SetMode::Not => Flags(current.0 & !flags.0),
        };
    }

    /// Push new flags, evicting the oldest entry when full.
    pub fn push(&mut self, flags: Flags) {
        self.index = (self.index + 1) % self.flags.len();
        self.flags[self.index] = flags;
    }

    /// Clear and pop entries; eight or more resets the entire history.
    pub fn pop(&mut self, count: usize) {
        if count >= self.flags.len() {
            *self = Self::default();
            return;
        }
        for _ in 0..count {
            self.flags[self.index] = Flags::DISABLED;
            self.index = (self.index + self.flags.len() - 1) % self.flags.len();
        }
    }
}

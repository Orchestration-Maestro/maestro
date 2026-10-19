//! Terminal modes and DECRPM responses.

/// Declare the mode catalogue together with its numbers and initial values.
macro_rules! modes {
    ($( $(#[$doc:meta])* $name:ident = ($number:literal, $ansi:literal, $default:literal)),+ $(,)?) => {
        /// A supported terminal mode; numbers are scoped to ANSI or private sequences.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Mode {
            $($(#[$doc])* $name),+
        }

        impl Mode {
            /// Resolve a mode number in the selected namespace.
            #[must_use]
            pub fn from_number(number: u16, ansi: bool) -> Option<Self> {
                match (number, ansi) {
                    $(($number, $ansi) => Some(Self::$name),)+
                    _ => None,
                }
            }

            /// Return the numeric tag used in control sequences.
            #[must_use]
            pub const fn tag(self) -> ModeTag {
                match self {
                    $(Self::$name => ModeTag { value: $number, ansi: $ansi }),+
                }
            }
        }

        /// Initial values indexed by the typed mode, independent of protocol numbers.
        const DEFAULTS: [bool; 41] = [$($default),+];
    };
}

modes! {
    /// Keyboard action mode, initially disabled.
    DisableKeyboard = (2, true, false),
    /// Insert rather than replace cells, initially disabled.
    Insert = (4, true, false),
    /// Send/receive mode, initially enabled.
    SendReceiveMode = (12, true, true),
    /// Automatic carriage return on linefeed, initially disabled.
    Linefeed = (20, true, false),
    /// Application cursor keys, initially disabled.
    CursorKeys = (1, false, false),
    /// 132-column mode, initially disabled.
    Column132 = (3, false, false),
    /// Slow scrolling, initially disabled.
    SlowScroll = (4, false, false),
    /// Reverse screen colours, initially disabled.
    ReverseColors = (5, false, false),
    /// Origin-relative cursor addressing, initially disabled.
    Origin = (6, false, false),
    /// Automatic wrapping, initially enabled.
    Wraparound = (7, false, true),
    /// Keyboard autorepeat, initially disabled.
    Autorepeat = (8, false, false),
    /// X10 mouse reporting, initially disabled.
    MouseEventX10 = (9, false, false),
    /// Cursor blinking, initially disabled.
    CursorBlinking = (12, false, false),
    /// Visible cursor, initially enabled.
    CursorVisible = (25, false, true),
    /// Allow column-mode changes, initially disabled.
    EnableMode3 = (40, false, false),
    /// Reverse wrapping, initially disabled.
    ReverseWrap = (45, false, false),
    /// Legacy alternate screen, initially disabled.
    AltScreenLegacy = (47, false, false),
    /// Application keypad keys, initially disabled.
    KeypadKeys = (66, false, false),
    /// Backspace sends a backspace byte, initially disabled.
    BackarrowKeyMode = (67, false, false),
    /// Left and right margins, initially disabled.
    EnableLeftAndRightMargin = (69, false, false),
    /// Button mouse reporting, initially disabled.
    MouseEventNormal = (1000, false, false),
    /// Button-motion mouse reporting, initially disabled.
    MouseEventButton = (1002, false, false),
    /// All-motion mouse reporting, initially disabled.
    MouseEventAny = (1003, false, false),
    /// Focus event reporting, initially disabled.
    FocusEvent = (1004, false, false),
    /// UTF-8 mouse encoding, initially disabled.
    MouseFormatUtf8 = (1005, false, false),
    /// SGR mouse encoding, initially disabled.
    MouseFormatSgr = (1006, false, false),
    /// Alternate-screen mouse scrolling, initially enabled.
    MouseAlternateScroll = (1007, false, true),
    /// Urxvt mouse encoding, initially disabled.
    MouseFormatUrxvt = (1015, false, false),
    /// Pixel-coordinate SGR mouse encoding, initially disabled.
    MouseFormatSgrPixels = (1016, false, false),
    /// Ignore keypad mode with Num Lock, initially enabled.
    IgnoreKeypadWithNumlock = (1035, false, true),
    /// Prefix Alt keys with escape, initially enabled.
    AltEscPrefix = (1036, false, true),
    /// Alt sends escape, initially disabled.
    AltSendsEscape = (1039, false, false),
    /// Extended reverse wrapping, initially disabled.
    ReverseWrapExtended = (1045, false, false),
    /// Alternate screen, initially disabled.
    AltScreen = (1047, false, false),
    /// Save cursor, initially disabled.
    SaveCursor = (1048, false, false),
    /// Alternate screen with cursor saving and clearing, initially disabled.
    AltScreenSaveCursorClearEnter = (1049, false, false),
    /// Bracketed paste, initially disabled.
    BracketedPaste = (2004, false, false),
    /// Synchronized output, initially disabled.
    SynchronizedOutput = (2026, false, false),
    /// Grapheme clustering, initially disabled in the mode catalogue.
    GraphemeCluster = (2027, false, false),
    /// Colour-scheme reporting, initially disabled.
    ReportColorScheme = (2031, false, false),
    /// In-band size reporting, initially disabled.
    InBandSizeReports = (2048, false, false),
}

/// A mode number together with its namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeTag {
    /// The numeric mode identifier.
    pub value: u16,
    /// Whether the tag uses the ANSI rather than private namespace.
    pub ansi: bool,
}

/// Current, saved and reset-default values for terminal modes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeState {
    /// Current values indexed by mode.
    values: [bool; 41],
    /// Last saved values indexed by mode.
    saved: [bool; 41],
    /// Values restored by reset.
    pub(crate) defaults: [bool; 41],
}

impl Default for ModeState {
    fn default() -> Self {
        Self {
            values: DEFAULTS,
            saved: DEFAULTS,
            defaults: DEFAULTS,
        }
    }
}

impl ModeState {
    /// Restore reset defaults and clear saved values to catalogue defaults.
    pub fn reset(&mut self) {
        self.values = self.defaults;
        self.saved = DEFAULTS;
    }

    /// Change a supported mode's current value.
    pub fn set(&mut self, mode: Mode, value: bool) {
        self.values[mode as usize] = value;
    }

    /// Read a supported mode's current value.
    #[must_use]
    pub fn get(&self, mode: Mode) -> bool {
        self.values[mode as usize]
    }

    /// Replace the saved value for a mode with its current value.
    pub fn save(&mut self, mode: Mode) {
        self.saved[mode as usize] = self.get(mode);
    }

    /// Restore a mode's last saved value, or its catalogue default if unsaved.
    pub fn restore(&mut self, mode: Mode) -> bool {
        let value = self.saved[mode as usize];
        self.set(mode, value);
        value
    }
}

/// A DECRPM response for a numeric mode tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    /// The requested mode and namespace.
    pub tag: ModeTag,
    /// Its reported state.
    pub state: ReportState,
}

/// State values encoded in a DECRPM response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ReportState {
    /// The mode number is not supported in the selected namespace.
    NotRecognized = 0,
    /// The mode is enabled.
    Set = 1,
    /// The mode is disabled.
    Reset = 2,
    /// The mode is permanently enabled.
    PermanentlySet = 3,
    /// The mode is permanently disabled.
    PermanentlyReset = 4,
}

impl ModeState {
    /// Report a mode's current value, or an unrecognized numeric tag.
    #[must_use]
    pub fn get_report(&self, tag: ModeTag) -> Report {
        let state = match Mode::from_number(tag.value, tag.ansi) {
            Some(mode) if self.get(mode) => ReportState::Set,
            Some(_) => ReportState::Reset,
            None => ReportState::NotRecognized,
        };
        Report { tag, state }
    }
}

impl Report {
    /// Write the DECRPM control sequence.
    ///
    /// # Errors
    /// Returns the writer's original error if writing fails.
    pub fn encode<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        write!(
            writer,
            "\x1b[{}{};{}$y",
            if self.tag.ansi { "" } else { "?" },
            self.tag.value,
            self.state as u8
        )
    }
}

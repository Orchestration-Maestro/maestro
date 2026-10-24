//! Physical keyboard events with owned text independent of the input caller.

pub use super::key_mods::Mods;

/// The action associated with a keyboard event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyAction {
    /// The key was released.
    Release,
    /// The key was pressed.
    #[default]
    Press,
    /// The held key repeated.
    Repeat,
}

/// A layout-independent physical key, using the W3C code vocabulary.
///
/// Layout-dependent text is supplied separately in [`KeyEvent::utf8`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Key {
    /// The unidentified physical key.
    #[default]
    Unidentified,
    /// The backquote physical key.
    Backquote,
    /// The backslash physical key.
    Backslash,
    /// The bracket left physical key.
    BracketLeft,
    /// The bracket right physical key.
    BracketRight,
    /// The comma physical key.
    Comma,
    /// The digit 0 physical key.
    Digit0,
    /// The digit 1 physical key.
    Digit1,
    /// The digit 2 physical key.
    Digit2,
    /// The digit 3 physical key.
    Digit3,
    /// The digit 4 physical key.
    Digit4,
    /// The digit 5 physical key.
    Digit5,
    /// The digit 6 physical key.
    Digit6,
    /// The digit 7 physical key.
    Digit7,
    /// The digit 8 physical key.
    Digit8,
    /// The digit 9 physical key.
    Digit9,
    /// The equal physical key.
    Equal,
    /// The intl backslash physical key.
    IntlBackslash,
    /// The intl ro physical key.
    IntlRo,
    /// The intl yen physical key.
    IntlYen,
    /// The key a physical key.
    KeyA,
    /// The key b physical key.
    KeyB,
    /// The key c physical key.
    KeyC,
    /// The key d physical key.
    KeyD,
    /// The key e physical key.
    KeyE,
    /// The key f physical key.
    KeyF,
    /// The key g physical key.
    KeyG,
    /// The key h physical key.
    KeyH,
    /// The key i physical key.
    KeyI,
    /// The key j physical key.
    KeyJ,
    /// The key k physical key.
    KeyK,
    /// The key l physical key.
    KeyL,
    /// The key m physical key.
    KeyM,
    /// The key n physical key.
    KeyN,
    /// The key o physical key.
    KeyO,
    /// The key p physical key.
    KeyP,
    /// The key q physical key.
    KeyQ,
    /// The key r physical key.
    KeyR,
    /// The key s physical key.
    KeyS,
    /// The key t physical key.
    KeyT,
    /// The key u physical key.
    KeyU,
    /// The key v physical key.
    KeyV,
    /// The key w physical key.
    KeyW,
    /// The key x physical key.
    KeyX,
    /// The key y physical key.
    KeyY,
    /// The key z physical key.
    KeyZ,
    /// The minus physical key.
    Minus,
    /// The period physical key.
    Period,
    /// The quote physical key.
    Quote,
    /// The semicolon physical key.
    Semicolon,
    /// The slash physical key.
    Slash,
    /// The alt left physical key.
    AltLeft,
    /// The alt right physical key.
    AltRight,
    /// The backspace physical key.
    Backspace,
    /// The caps lock physical key.
    CapsLock,
    /// The context menu physical key.
    ContextMenu,
    /// The control left physical key.
    ControlLeft,
    /// The control right physical key.
    ControlRight,
    /// The enter physical key.
    Enter,
    /// The meta left physical key.
    MetaLeft,
    /// The meta right physical key.
    MetaRight,
    /// The shift left physical key.
    ShiftLeft,
    /// The shift right physical key.
    ShiftRight,
    /// The space physical key.
    Space,
    /// The tab physical key.
    Tab,
    /// The convert physical key.
    Convert,
    /// The kana mode physical key.
    KanaMode,
    /// The non convert physical key.
    NonConvert,
    /// The delete physical key.
    Delete,
    /// The end physical key.
    End,
    /// The help physical key.
    Help,
    /// The home physical key.
    Home,
    /// The insert physical key.
    Insert,
    /// The page down physical key.
    PageDown,
    /// The page up physical key.
    PageUp,
    /// The arrow down physical key.
    ArrowDown,
    /// The arrow left physical key.
    ArrowLeft,
    /// The arrow right physical key.
    ArrowRight,
    /// The arrow up physical key.
    ArrowUp,
    /// The num lock physical key.
    NumLock,
    /// The numpad 0 physical key.
    Numpad0,
    /// The numpad 1 physical key.
    Numpad1,
    /// The numpad 2 physical key.
    Numpad2,
    /// The numpad 3 physical key.
    Numpad3,
    /// The numpad 4 physical key.
    Numpad4,
    /// The numpad 5 physical key.
    Numpad5,
    /// The numpad 6 physical key.
    Numpad6,
    /// The numpad 7 physical key.
    Numpad7,
    /// The numpad 8 physical key.
    Numpad8,
    /// The numpad 9 physical key.
    Numpad9,
    /// The numpad add physical key.
    NumpadAdd,
    /// The numpad backspace physical key.
    NumpadBackspace,
    /// The numpad clear physical key.
    NumpadClear,
    /// The numpad clear entry physical key.
    NumpadClearEntry,
    /// The numpad comma physical key.
    NumpadComma,
    /// The numpad decimal physical key.
    NumpadDecimal,
    /// The numpad divide physical key.
    NumpadDivide,
    /// The numpad enter physical key.
    NumpadEnter,
    /// The numpad equal physical key.
    NumpadEqual,
    /// The numpad memory add physical key.
    NumpadMemoryAdd,
    /// The numpad memory clear physical key.
    NumpadMemoryClear,
    /// The numpad memory recall physical key.
    NumpadMemoryRecall,
    /// The numpad memory store physical key.
    NumpadMemoryStore,
    /// The numpad memory subtract physical key.
    NumpadMemorySubtract,
    /// The numpad multiply physical key.
    NumpadMultiply,
    /// The numpad paren left physical key.
    NumpadParenLeft,
    /// The numpad paren right physical key.
    NumpadParenRight,
    /// The numpad subtract physical key.
    NumpadSubtract,
    /// The numpad separator physical key.
    NumpadSeparator,
    /// The numpad up physical key.
    NumpadUp,
    /// The numpad down physical key.
    NumpadDown,
    /// The numpad right physical key.
    NumpadRight,
    /// The numpad left physical key.
    NumpadLeft,
    /// The numpad begin physical key.
    NumpadBegin,
    /// The numpad home physical key.
    NumpadHome,
    /// The numpad end physical key.
    NumpadEnd,
    /// The numpad insert physical key.
    NumpadInsert,
    /// The numpad delete physical key.
    NumpadDelete,
    /// The numpad page up physical key.
    NumpadPageUp,
    /// The numpad page down physical key.
    NumpadPageDown,
    /// The escape physical key.
    Escape,
    /// The f1 physical key.
    F1,
    /// The f2 physical key.
    F2,
    /// The f3 physical key.
    F3,
    /// The f4 physical key.
    F4,
    /// The f5 physical key.
    F5,
    /// The f6 physical key.
    F6,
    /// The f7 physical key.
    F7,
    /// The f8 physical key.
    F8,
    /// The f9 physical key.
    F9,
    /// The f10 physical key.
    F10,
    /// The f11 physical key.
    F11,
    /// The f12 physical key.
    F12,
    /// The f13 physical key.
    F13,
    /// The f14 physical key.
    F14,
    /// The f15 physical key.
    F15,
    /// The f16 physical key.
    F16,
    /// The f17 physical key.
    F17,
    /// The f18 physical key.
    F18,
    /// The f19 physical key.
    F19,
    /// The f20 physical key.
    F20,
    /// The f21 physical key.
    F21,
    /// The f22 physical key.
    F22,
    /// The f23 physical key.
    F23,
    /// The f24 physical key.
    F24,
    /// The f25 physical key.
    F25,
    /// The fn physical key.
    Fn,
    /// The fn lock physical key.
    FnLock,
    /// The print screen physical key.
    PrintScreen,
    /// The scroll lock physical key.
    ScrollLock,
    /// The pause physical key.
    Pause,
    /// The browser back physical key.
    BrowserBack,
    /// The browser favorites physical key.
    BrowserFavorites,
    /// The browser forward physical key.
    BrowserForward,
    /// The browser home physical key.
    BrowserHome,
    /// The browser refresh physical key.
    BrowserRefresh,
    /// The browser search physical key.
    BrowserSearch,
    /// The browser stop physical key.
    BrowserStop,
    /// The eject physical key.
    Eject,
    /// The launch app 1 physical key.
    LaunchApp1,
    /// The launch app 2 physical key.
    LaunchApp2,
    /// The launch mail physical key.
    LaunchMail,
    /// The media play pause physical key.
    MediaPlayPause,
    /// The media select physical key.
    MediaSelect,
    /// The media stop physical key.
    MediaStop,
    /// The media track next physical key.
    MediaTrackNext,
    /// The media track previous physical key.
    MediaTrackPrevious,
    /// The power physical key.
    Power,
    /// The sleep physical key.
    Sleep,
    /// The audio volume down physical key.
    AudioVolumeDown,
    /// The audio volume mute physical key.
    AudioVolumeMute,
    /// The audio volume up physical key.
    AudioVolumeUp,
    /// The wake up physical key.
    WakeUp,
    /// The copy physical key.
    Copy,
    /// The cut physical key.
    Cut,
    /// The paste physical key.
    Paste,
}

/// Input supplied by the caller to encode a key for a terminal application.
///
/// Text is owned: changing or dropping the caller's buffer cannot change it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyEvent {
    /// Press, release or repeat.
    pub action: KeyAction,
    /// The physical key, independent of the keyboard layout.
    pub key: Key,
    /// Pressed modifiers, locks and observed sides.
    pub mods: Mods,
    /// Modifiers consumed to produce the text; ignored when text is empty.
    pub consumed_mods: Mods,
    /// Whether this is an intermediate dead-key composition event.
    pub composing: bool,
    /// Generated UTF-8 text, or preedit text during composition.
    pub utf8: String,
    /// The unshifted codepoint, or zero when not supplied.
    pub unshifted_codepoint: u32,
}

impl KeyEvent {
    /// Copy caller-supplied text into the event's own storage.
    pub fn set_utf8(&mut self, text: &str) {
        self.utf8.clear();
        self.utf8.push_str(text);
    }

    /// Remove consumed modifiers only when the event generated text.
    #[must_use]
    pub fn effective_mods(&self) -> Mods {
        if self.utf8.is_empty() {
            self.mods
        } else {
            self.mods.unset(self.consumed_mods)
        }
    }
}

impl Key {
    /// Resolve a numbered function key without a caller-owned key table.
    #[must_use]
    pub fn function(number: u8) -> Option<Self> {
        const KEYS: [Key; 12] = [
            Key::F1,
            Key::F2,
            Key::F3,
            Key::F4,
            Key::F5,
            Key::F6,
            Key::F7,
            Key::F8,
            Key::F9,
            Key::F10,
            Key::F11,
            Key::F12,
        ];
        KEYS.get(usize::from(number.checked_sub(1)?)).copied()
    }

    /// Return the base-layout codepoint, including numeric keypad characters.
    #[must_use]
    pub fn codepoint(self) -> Option<char> {
        match self {
            Self::KeyA => Some('a'),
            Self::KeyB => Some('b'),
            Self::KeyC => Some('c'),
            Self::KeyD => Some('d'),
            Self::KeyE => Some('e'),
            Self::KeyF => Some('f'),
            Self::KeyG => Some('g'),
            Self::KeyH => Some('h'),
            Self::KeyI => Some('i'),
            Self::KeyJ => Some('j'),
            Self::KeyK => Some('k'),
            Self::KeyL => Some('l'),
            Self::KeyM => Some('m'),
            Self::KeyN => Some('n'),
            Self::KeyO => Some('o'),
            Self::KeyP => Some('p'),
            Self::KeyQ => Some('q'),
            Self::KeyR => Some('r'),
            Self::KeyS => Some('s'),
            Self::KeyT => Some('t'),
            Self::KeyU => Some('u'),
            Self::KeyV => Some('v'),
            Self::KeyW => Some('w'),
            Self::KeyX => Some('x'),
            Self::KeyY => Some('y'),
            Self::KeyZ => Some('z'),
            Self::Digit0 => Some('0'),
            Self::Digit1 => Some('1'),
            Self::Digit2 => Some('2'),
            Self::Digit3 => Some('3'),
            Self::Digit4 => Some('4'),
            Self::Digit5 => Some('5'),
            Self::Digit6 => Some('6'),
            Self::Digit7 => Some('7'),
            Self::Digit8 => Some('8'),
            Self::Digit9 => Some('9'),
            Self::Semicolon => Some(';'),
            Self::Space => Some(' '),
            Self::Quote => Some('\''),
            Self::Comma => Some(','),
            Self::Backquote => Some('`'),
            Self::Period => Some('.'),
            Self::Slash => Some('/'),
            Self::Minus => Some('-'),
            Self::Equal => Some('='),
            Self::BracketLeft => Some('['),
            Self::BracketRight => Some(']'),
            Self::Backslash => Some('\\'),
            Self::Tab => Some('\t'),
            _ => self.keypad_codepoint(),
        }
    }

    /// Return the base codepoint for a numeric keypad key.
    fn keypad_codepoint(self) -> Option<char> {
        match self {
            Self::Numpad0 => Some('0'),
            Self::Numpad1 => Some('1'),
            Self::Numpad2 => Some('2'),
            Self::Numpad3 => Some('3'),
            Self::Numpad4 => Some('4'),
            Self::Numpad5 => Some('5'),
            Self::Numpad6 => Some('6'),
            Self::Numpad7 => Some('7'),
            Self::Numpad8 => Some('8'),
            Self::Numpad9 => Some('9'),
            Self::NumpadDecimal => Some('.'),
            Self::NumpadDivide => Some('/'),
            Self::NumpadMultiply => Some('*'),
            Self::NumpadSubtract => Some('-'),
            Self::NumpadAdd => Some('+'),
            Self::NumpadEqual => Some('='),
            _ => None,
        }
    }
}

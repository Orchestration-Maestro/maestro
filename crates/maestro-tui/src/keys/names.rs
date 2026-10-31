//! Chat key identifiers and structured logical key vocabulary.

/// A textual key identifier such as `"ctrl+c"`, `"escape"` or `"shift+tab"`.
///
/// The identifier is open data: any string is accepted by the matching operations and
/// unsupported ones simply never match. [`Key`] builds the supported spellings.
pub type KeyId = String;

/// Modifier bit for shift in the enhanced protocol after subtracting one.
pub(super) const SHIFT: u32 = 1;
/// Modifier bit for alt.
pub(super) const ALT: u32 = 2;
/// Modifier bit for ctrl.
pub(super) const CTRL: u32 = 4;
/// Modifier bit for super.
pub(super) const SUPER: u32 = 8;
/// Modifier bit for hyper.
pub(super) const HYPER: u32 = 16;
/// Caps Lock and Num Lock bits, which naming, typed text and enhanced matching ignore.
pub(super) const LOCKS: u32 = 64 | 128;

/// Printable symbols accepted as key identifiers.
const SYMBOLS: &str = "`-=[]\\;',./!@#$%^&*()_+|~{}:<>?";

/// The Numpad Enter code of the enhanced protocol: it is named `enter` but matched as a code of its own.
pub(super) const KEYPAD_ENTER: char = '\u{e046}';

/// Enhanced-protocol keypad codes and the logical keys they stand for.
const KEYPAD: [(u32, Code); 27] = [
    (57399, Code::Char('0')),
    (57400, Code::Char('1')),
    (57401, Code::Char('2')),
    (57402, Code::Char('3')),
    (57403, Code::Char('4')),
    (57404, Code::Char('5')),
    (57405, Code::Char('6')),
    (57406, Code::Char('7')),
    (57407, Code::Char('8')),
    (57408, Code::Char('9')),
    (57409, Code::Char('.')),
    (57410, Code::Char('/')),
    (57411, Code::Char('*')),
    (57412, Code::Char('-')),
    (57413, Code::Char('+')),
    (57415, Code::Char('=')),
    (57416, Code::Char(',')),
    (57417, Code::Key(Named::Left)),
    (57418, Code::Key(Named::Right)),
    (57419, Code::Key(Named::Up)),
    (57420, Code::Key(Named::Down)),
    (57421, Code::Key(Named::PageUp)),
    (57422, Code::Key(Named::PageDown)),
    (57423, Code::Key(Named::Home)),
    (57424, Code::Key(Named::End)),
    (57425, Code::Key(Named::Insert)),
    (57426, Code::Key(Named::Delete)),
];

/// Whether `c` is an ASCII lowercase letter, a digit or one of the symbols usable in key
/// identifiers.
fn is_key_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || SYMBOLS.contains(c)
}

/// The control character a terminal sends for ctrl plus `key`, if there is one.
pub(super) fn raw_ctrl_char(key: char) -> Option<char> {
    let code = u8::try_from(key).ok()?;
    match key {
        'a'..='z' | '[' | '\\' | ']' | '_' => Some(char::from(code & 0x1f)),
        '-' => Some('\u{1f}'),
        _ => None,
    }
}

/// Lowers an ASCII capital when shift is held, which reports capitals as shifted lowercase.
pub(super) fn shift_lowered(c: char, modifier: u32) -> char {
    if modifier & SHIFT != 0 {
        c.to_ascii_lowercase()
    } else {
        c
    }
}

/// Builds `modifiers+name`, or `None` when the modifier bits are not all supported.
///
/// Lock bits are ignored; the modifier names always come in the order shift, ctrl, alt, super.
pub(super) fn with_modifiers(name: &str, modifier: u32) -> Option<KeyId> {
    let effective = modifier & !LOCKS;
    if effective & !(SHIFT | ALT | CTRL | SUPER) != 0 {
        return None;
    }
    let mut id = String::new();
    for (bit, label) in [
        (SHIFT, "shift+"),
        (CTRL, "ctrl+"),
        (ALT, "alt+"),
        (SUPER, "super+"),
    ] {
        if effective & bit != 0 {
            id.push_str(label);
        }
    }
    id.push_str(name);
    Some(id)
}

/// A named logical key; only the chat subset has an identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Named {
    /// The escape key.
    Escape,
    /// The tab key.
    Tab,
    /// Reverse tab navigation.
    BackTab,
    /// The enter key.
    Enter,
    /// The space bar.
    Space,
    /// The backspace key.
    Backspace,
    /// The clear key, which the enhanced protocol never reports.
    Clear,
    /// The insert key.
    Insert,
    /// The delete key.
    Delete,
    /// The home key.
    Home,
    /// The end key.
    End,
    /// The page-up key.
    PageUp,
    /// The page-down key.
    PageDown,
    /// The up arrow.
    Up,
    /// The down arrow.
    Down,
    /// The left arrow.
    Left,
    /// The right arrow.
    Right,
    /// The `CapsLock` functional key.
    CapsLock,
    /// The `ScrollLock` functional key.
    ScrollLock,
    /// The `NumLock` functional key.
    NumLock,
    /// The `PrintScreen` functional key.
    PrintScreen,
    /// The `Pause` functional key.
    Pause,
    /// The `Menu` functional key.
    Menu,
    /// The `KeypadBegin` functional key.
    KeypadBegin,
    /// The `MediaPlay` functional key.
    MediaPlay,
    /// The `MediaPause` functional key.
    MediaPause,
    /// The `MediaPlayPause` functional key.
    MediaPlayPause,
    /// The `MediaReverse` functional key.
    MediaReverse,
    /// The `MediaStop` functional key.
    MediaStop,
    /// The `MediaFastForward` functional key.
    MediaFastForward,
    /// The `MediaRewind` functional key.
    MediaRewind,
    /// The `MediaTrackNext` functional key.
    MediaTrackNext,
    /// The `MediaTrackPrevious` functional key.
    MediaTrackPrevious,
    /// The `MediaRecord` functional key.
    MediaRecord,
    /// The `MediaLowerVolume` functional key.
    MediaLowerVolume,
    /// The `MediaRaiseVolume` functional key.
    MediaRaiseVolume,
    /// The `MediaMuteVolume` functional key.
    MediaMuteVolume,
    /// The `LeftShift` functional key.
    LeftShift,
    /// The `LeftControl` functional key.
    LeftControl,
    /// The `LeftAlt` functional key.
    LeftAlt,
    /// The `LeftSuper` functional key.
    LeftSuper,
    /// The `LeftHyper` functional key.
    LeftHyper,
    /// The `LeftMeta` functional key.
    LeftMeta,
    /// The `RightShift` functional key.
    RightShift,
    /// The `RightControl` functional key.
    RightControl,
    /// The `RightAlt` functional key.
    RightAlt,
    /// The `RightSuper` functional key.
    RightSuper,
    /// The `RightHyper` functional key.
    RightHyper,
    /// The `RightMeta` functional key.
    RightMeta,
    /// The `IsoLevel3Shift` functional key.
    IsoLevel3Shift,
    /// The `IsoLevel5Shift` functional key.
    IsoLevel5Shift,
    /// A function key, numbered from 1 to 35.
    Function(u8),
}

impl Named {
    /// Maps native functional code points without extending chat identifiers.
    fn host(codepoint: u32) -> Option<Self> {
        Some(match codepoint {
            57358 => Self::CapsLock,
            57359 => Self::ScrollLock,
            57360 => Self::NumLock,
            57361 => Self::PrintScreen,
            57362 => Self::Pause,
            57363 => Self::Menu,
            57427 => Self::KeypadBegin,
            57428 => Self::MediaPlay,
            57429 => Self::MediaPause,
            57430 => Self::MediaPlayPause,
            57431 => Self::MediaReverse,
            57432 => Self::MediaStop,
            57433 => Self::MediaFastForward,
            57434 => Self::MediaRewind,
            57435 => Self::MediaTrackNext,
            57436 => Self::MediaTrackPrevious,
            57437 => Self::MediaRecord,
            57438 => Self::MediaLowerVolume,
            57439 => Self::MediaRaiseVolume,
            57440 => Self::MediaMuteVolume,
            57441 => Self::LeftShift,
            57442 => Self::LeftControl,
            57443 => Self::LeftAlt,
            57444 => Self::LeftSuper,
            57445 => Self::LeftHyper,
            57446 => Self::LeftMeta,
            57447 => Self::RightShift,
            57448 => Self::RightControl,
            57449 => Self::RightAlt,
            57450 => Self::RightSuper,
            57451 => Self::RightHyper,
            57452 => Self::RightMeta,
            57453 => Self::IsoLevel3Shift,
            57454 => Self::IsoLevel5Shift,
            57376..=57398 => Self::Function(u8::try_from(codepoint - 57376 + 13).ok()?),
            _ => return None,
        })
    }

    /// Finds the key an already lowercased identifier names, including its aliases.
    pub(super) fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "escape" | "esc" => Self::Escape,
            "tab" => Self::Tab,
            "enter" | "return" => Self::Enter,
            "space" => Self::Space,
            "backspace" | "bs" => Self::Backspace,
            "clear" => Self::Clear,
            "insert" => Self::Insert,
            "delete" => Self::Delete,
            "home" => Self::Home,
            "end" => Self::End,
            "pageup" => Self::PageUp,
            "pagedown" => Self::PageDown,
            "up" => Self::Up,
            "down" => Self::Down,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => return Self::parse_function(name),
        })
    }

    /// Finds the function key an identifier from `f0` to `f255` names.
    fn parse_function(name: &str) -> Option<Self> {
        name.strip_prefix('f')
            .filter(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|digits| digits.parse().ok())
            .map(Self::Function)
    }

    /// The canonical identifier of the key.
    pub(super) fn name(self) -> Option<String> {
        let fixed = match self {
            Self::Escape => "escape",
            Self::Tab => "tab",
            Self::Enter => "enter",
            Self::Space => "space",
            Self::Backspace => "backspace",
            Self::Clear => "clear",
            Self::Insert => "insert",
            Self::Delete => "delete",
            Self::Home => "home",
            Self::End => "end",
            Self::PageUp => "pageUp",
            Self::PageDown => "pageDown",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Function(number @ 1..=12) => return Some(format!("f{number}")),
            _ => return None,
        };
        Some(fixed.to_owned())
    }

    /// The enhanced-protocol code of the key, absent for clear and the function keys.
    pub(super) const fn code(self) -> Option<Code> {
        match self {
            Self::Escape => Some(Code::Char('\x1b')),
            Self::Tab => Some(Code::Char('\t')),
            Self::Enter => Some(Code::Char('\r')),
            Self::Space => Some(Code::Char(' ')),
            Self::Backspace => Some(Code::Char('\x7f')),
            Self::Clear | Self::Function(_) => None,
            navigation => Some(Code::Key(navigation)),
        }
    }
}

/// The logical key an enhanced-protocol report names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    /// A key identified by the character it produces or its Unicode code.
    Char(char),
    /// A named navigation, editing or native functional key.
    Key(Named),
}

impl Code {
    /// Converts a host code point into its structured logical key.
    pub(super) fn host(codepoint: u32) -> Option<Self> {
        if let Some(key) = Named::host(codepoint) {
            return Some(Self::Key(key));
        }
        Some(match codepoint {
            8 | 127 => Self::Key(Named::Backspace),
            9 => Self::Key(Named::Tab),
            13 | 57414 => Self::Key(Named::Enter),
            27 => Self::Key(Named::Escape),
            value => {
                let raw = Self::Char(char::from_u32(value)?);
                let code = raw.normalized();
                if (57358..=57454).contains(&value) && code == raw {
                    return None;
                }
                code
            }
        })
    }

    /// Maps keypad codes onto the digits, symbols and navigation keys they stand for.
    pub(super) fn normalized(self) -> Self {
        let Self::Char(c) = self else { return self };
        KEYPAD
            .iter()
            .find(|(code, _)| *code == u32::from(c))
            .map_or(self, |(_, key)| *key)
    }

    /// The identity used for comparison: keypad codes mapped, shifted capitals lowered.
    pub(super) fn identity(self, modifier: u32) -> Self {
        match self.normalized() {
            Self::Char(c) => Self::Char(shift_lowered(c, modifier)),
            key @ Self::Key(_) => key,
        }
    }

    /// Whether the code is an ASCII lowercase letter, digit or symbol, which a base-layout
    /// key may not replace.
    pub(super) fn is_authoritative(self) -> bool {
        matches!(self, Self::Char(c) if is_key_char(c))
    }

    /// The identifier of the key, absent when it has none.
    pub(super) fn name(self) -> Option<String> {
        match self {
            Self::Key(key) => key.name(),
            Self::Char(c) => match c {
                '\x1b' => Named::Escape.name(),
                '\t' => Named::Tab.name(),
                '\r' | KEYPAD_ENTER => Named::Enter.name(),
                ' ' => Named::Space.name(),
                '\x7f' => Named::Backspace.name(),
                _ if self.is_authoritative() => Some(c.to_string()),
                _ => None,
            },
        }
    }
}

/// What a key identifier asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A named key.
    Named(Named),
    /// A single character as typed, except that an ASCII capital is its lowercase letter.
    Char(char),
}

/// The symbol a word such as `minus` names.
fn symbol_word(word: &str) -> Option<char> {
    Some(match word {
        "minus" => '-',
        "comma" => ',',
        "period" => '.',
        "slash" => '/',
        "backslash" => '\\',
        "quote" => '\'',
        "double_quote" | "double-quote" => '"',
        "semicolon" => ';',
        "colon" => ':',
        "percent" => '%',
        "ampersand" => '&',
        "backtick" => '`',
        "plus" => '+',
        _ => return None,
    })
}

/// The modifier bit a modifier word names, including its aliases.
fn modifier_bit(word: &str) -> Option<u32> {
    Some(match word {
        "shift" => SHIFT,
        "alt" | "option" | "meta" => ALT,
        "ctrl" | "control" => CTRL,
        "super" | "cmd" | "command" => SUPER,
        "hyper" => HYPER,
        _ => return None,
    })
}

/// A parsed key identifier: its key and the modifier bits it requires.
///
/// The workspace keybindings and the chat keybindings parse their spellings through this
/// one grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identifier {
    /// The requested key.
    pub target: Target,
    /// The modifier bits it requires: shift 1, alt 2, ctrl 4, super 8 and hyper 16.
    pub modifier: u32,
    /// Whether the key was written as an ASCII capital. The identifier names the lowercase
    /// letter; a frontend that reads a capital as shifted adds shift itself.
    pub capital: bool,
}

impl Identifier {
    /// Parses an identifier.
    ///
    /// The key is the text after the last `+`, except that `+` alone and an identifier
    /// ending in `++` name the plus key. Surrounding whitespace is ignored around each word.
    /// The words before the key are modifier names in any letter case: `shift`, `ctrl`
    /// (`control`), `alt` (`option`, `meta`), `super` (`cmd`, `command`) and `hyper`. An
    /// identifier with a word that is not a modifier name, with an empty word (as in `ctrl+`
    /// or `ctrl++v`) or with no key names nothing, so a misspelled modifier never binds the
    /// plain key. A key of one character is that character as typed, except that an ASCII
    /// capital names its lowercase letter and sets `capital`; any other key is a named key
    /// or symbol word in any letter case.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        let id = id.trim();
        let (words, key) = if id == "+" {
            (None, "+")
        } else if let Some(words) = id.strip_suffix("++") {
            (Some(words), "+")
        } else {
            match id.rsplit_once('+') {
                Some((words, key)) => (Some(words), key),
                None => (None, id),
            }
        };
        let modifier = words.map_or(Some(0), |words| {
            words.split('+').try_fold(0, |bits, word| {
                Some(bits | modifier_bit(&word.trim().to_lowercase())?)
            })
        })?;
        let key = key.trim();
        let mut chars = key.chars();
        let (target, capital) = if let (Some(c), None) = (chars.next(), chars.next()) {
            (Target::Char(c.to_ascii_lowercase()), c.is_ascii_uppercase())
        } else {
            let word = key.to_lowercase();
            let target = Named::parse(&word)
                .map(Target::Named)
                .or_else(|| symbol_word(&word).map(Target::Char))?;
            (target, false)
        };
        Some(Self {
            target,
            modifier,
            capital,
        })
    }
}

/// Defines the string constants of [`Key`].
macro_rules! key_names {
    ($($name:ident = $text:literal;)*) => {
        $(
            #[doc = concat!("The key identifier \"", $text, "\".")]
            pub const $name: &'static str = $text;
        )*
    };
}

/// Defines the modifier constructors of [`Key`].
macro_rules! key_modifiers {
    ($($name:ident = $prefix:literal;)*) => {
        $(
            #[doc = concat!("Builds the identifier \"", $prefix, "\" followed by `key`.")]
            #[must_use]
            pub fn $name(key: &str) -> KeyId {
                format!(concat!($prefix, "{}"), key)
            }
        )*
    };
}

/// Spellings of key identifiers, for callers that prefer names to string literals.
///
/// ```
/// use maestro_tui::Key;
///
/// assert_eq!(Key::ESCAPE, "escape");
/// assert_eq!(Key::ctrl("c"), "ctrl+c");
/// assert_eq!(Key::ctrl_shift(Key::PIPE), "ctrl+shift+|");
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Key;

impl Key {
    key_names! {
        ESCAPE = "escape";
        ESC = "esc";
        ENTER = "enter";
        RETURN = "return";
        TAB = "tab";
        SPACE = "space";
        BACKSPACE = "backspace";
        DELETE = "delete";
        INSERT = "insert";
        CLEAR = "clear";
        HOME = "home";
        END = "end";
        PAGE_UP = "pageUp";
        PAGE_DOWN = "pageDown";
        UP = "up";
        DOWN = "down";
        LEFT = "left";
        RIGHT = "right";
        F1 = "f1";
        F2 = "f2";
        F3 = "f3";
        F4 = "f4";
        F5 = "f5";
        F6 = "f6";
        F7 = "f7";
        F8 = "f8";
        F9 = "f9";
        F10 = "f10";
        F11 = "f11";
        F12 = "f12";
        BACKTICK = "`";
        HYPHEN = "-";
        EQUALS = "=";
        LEFTBRACKET = "[";
        RIGHTBRACKET = "]";
        BACKSLASH = "\\";
        SEMICOLON = ";";
        QUOTE = "'";
        COMMA = ",";
        PERIOD = ".";
        SLASH = "/";
        EXCLAMATION = "!";
        AT = "@";
        HASH = "#";
        DOLLAR = "$";
        PERCENT = "%";
        CARET = "^";
        AMPERSAND = "&";
        ASTERISK = "*";
        LEFTPAREN = "(";
        RIGHTPAREN = ")";
        UNDERSCORE = "_";
        PLUS = "+";
        PIPE = "|";
        TILDE = "~";
        LEFTBRACE = "{";
        RIGHTBRACE = "}";
        COLON = ":";
        LESSTHAN = "<";
        GREATERTHAN = ">";
        QUESTION = "?";
    }

    key_modifiers! {
        ctrl = "ctrl+";
        shift = "shift+";
        alt = "alt+";
        super_key = "super+";
        ctrl_shift = "ctrl+shift+";
        shift_ctrl = "shift+ctrl+";
        ctrl_alt = "ctrl+alt+";
        alt_ctrl = "alt+ctrl+";
        shift_alt = "shift+alt+";
        alt_shift = "alt+shift+";
        ctrl_super = "ctrl+super+";
        super_ctrl = "super+ctrl+";
        shift_super = "shift+super+";
        super_shift = "super+shift+";
        alt_super = "alt+super+";
        super_alt = "super+alt+";
        ctrl_shift_alt = "ctrl+shift+alt+";
        ctrl_shift_super = "ctrl+shift+super+";
    }
}

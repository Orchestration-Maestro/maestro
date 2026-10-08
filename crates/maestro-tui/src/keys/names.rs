//! Key identifiers: the public `Key` helper and the private key vocabulary.

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

/// A key with a name of its own in identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Named {
    /// The escape key.
    Escape,
    /// The tab key.
    Tab,
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
    /// A function key, numbered from 1 to 12.
    Function(u8),
}

impl Named {
    /// Finds the key an already lowercased identifier names, including its aliases.
    pub(super) fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "escape" | "esc" => Self::Escape,
            "tab" => Self::Tab,
            "enter" | "return" => Self::Enter,
            "space" => Self::Space,
            "backspace" => Self::Backspace,
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

    /// Finds the function key an identifier from `f1` to `f12` names.
    fn parse_function(name: &str) -> Option<Self> {
        (1..=12)
            .find(|number| name == format!("f{number}"))
            .map(Self::Function)
    }

    /// The canonical identifier of the key.
    pub(super) fn name(self) -> String {
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
            Self::Function(number) => return format!("f{number}"),
        };
        fixed.to_owned()
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
pub(super) enum Code {
    /// A key identified by the character it produces or its Unicode code.
    Char(char),
    /// A navigation or editing key.
    Key(Named),
}

impl Code {
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
            Self::Key(key) => Some(key.name()),
            Self::Char(c) => match c {
                '\x1b' => Some(Named::Escape.name()),
                '\t' => Some(Named::Tab.name()),
                '\r' | KEYPAD_ENTER => Some(Named::Enter.name()),
                ' ' => Some(Named::Space.name()),
                '\x7f' => Some(Named::Backspace.name()),
                _ if self.is_authoritative() => Some(c.to_string()),
                _ => None,
            },
        }
    }
}

/// What a key identifier asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    /// A named key.
    Named(Named),
    /// A lowercase letter, a digit or a symbol.
    Char(char),
}

/// A parsed key identifier: its key and the modifier bits it requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Identifier {
    /// The requested key.
    pub(super) target: Target,
    /// The modifier bits it requires, built from shift, alt, ctrl and super.
    pub(super) modifier: u32,
}

impl Identifier {
    /// Parses an identifier, lowercasing it as Unicode defines, so a capital such as the
    /// Kelvin sign names the ASCII letter it lowercases to.
    ///
    /// The key is the text after the last `+`, except that `+` alone and an identifier
    /// ending in `++` name the plus key. An identifier ending in a single `+` after a word
    /// has an empty key and names nothing. Words before the key that are not modifier names
    /// are ignored.
    pub(super) fn parse(id: &str) -> Option<Self> {
        let id = id.to_lowercase();
        let (prefix, key) = if id == "+" {
            ("", "+")
        } else if let Some(prefix) = id.strip_suffix("++") {
            (prefix, "+")
        } else {
            id.rsplit_once('+').unwrap_or(("", &id))
        };
        let mut chars = key.chars();
        let target = match (chars.next(), chars.next()) {
            (Some(c), None) if is_key_char(c) => Target::Char(c),
            _ => Target::Named(Named::parse(key)?),
        };
        let modifier = prefix.split('+').fold(0, |bits, word| {
            bits | match word {
                "shift" => SHIFT,
                "alt" => ALT,
                "ctrl" => CTRL,
                "super" => SUPER,
                _ => 0,
            }
        });
        Some(Self { target, modifier })
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

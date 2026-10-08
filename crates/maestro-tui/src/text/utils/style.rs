//! Typed terminal style state: attributes, colours and hyperlinks.

/// Closing sequence of an OSC 8 hyperlink, kept as the opener spelled it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Terminator {
    /// Bell character.
    Bel,
    /// String terminator (`ESC \`).
    St,
}

impl Terminator {
    /// Source spelling of the terminator.
    const fn spelling(self) -> &'static str {
        match self {
            Self::Bel => "\x07",
            Self::St => "\x1b\\",
        }
    }
}

/// What an OSC 8 escape does to the active hyperlink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LinkChange<'a> {
    /// Opens a link, replacing any active one.
    Open(Hyperlink<'a>),
    /// Closes the active link.
    Close,
}

/// An open OSC 8 hyperlink with the exact parameters and terminator of its opener.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Hyperlink<'a> {
    /// Text between the second and third semicolon of the opener.
    params: &'a str,
    /// Target of the link; never empty for an open link.
    uri: &'a str,
    /// Terminator the opener used.
    terminator: Terminator,
}

impl<'a> Hyperlink<'a> {
    /// Interprets an OSC 8 escape; `None` for any other escape.
    pub(super) fn parse(code: &'a str) -> Option<LinkChange<'a>> {
        let body = code.strip_prefix("\x1b]8;")?;
        let (body, terminator) = match body.strip_suffix('\x07') {
            Some(body) => (body, Terminator::Bel),
            None => (body.strip_suffix("\x1b\\")?, Terminator::St),
        };
        let (params, uri) = body.split_once(';')?;
        Some(if uri.is_empty() {
            LinkChange::Close
        } else {
            LinkChange::Open(Self {
                params,
                uri,
                terminator,
            })
        })
    }

    /// Appends the opening escape.
    fn write_open(self, out: &mut String) {
        out.push_str("\x1b]8;");
        out.push_str(self.params);
        out.push(';');
        out.push_str(self.uri);
        out.push_str(self.terminator.spelling());
    }

    /// Appends the closing escape in the opener's terminator.
    fn write_close(self, out: &mut String) {
        out.push_str("\x1b]8;;");
        out.push_str(self.terminator.spelling());
    }
}

/// Boolean text attributes in canonical emission order.
#[derive(Clone, Copy)]
enum Attribute {
    /// Bold or increased intensity (SGR 1).
    Bold,
    /// Dim or decreased intensity (SGR 2).
    Dim,
    /// Italic (SGR 3).
    Italic,
    /// Underline (SGR 4).
    Underline,
    /// Blink (SGR 5).
    Blink,
    /// Swapped foreground and background (SGR 7).
    Inverse,
    /// Hidden text (SGR 8).
    Hidden,
    /// Crossed-out text (SGR 9).
    Strikethrough,
}

impl Attribute {
    /// Every attribute, in the order active codes are written.
    const ALL: [Self; 8] = [
        Self::Bold,
        Self::Dim,
        Self::Italic,
        Self::Underline,
        Self::Blink,
        Self::Inverse,
        Self::Hidden,
        Self::Strikethrough,
    ];

    /// Bit in the attribute mask.
    const fn bit(self) -> u8 {
        1 << self as u8
    }

    /// SGR parameter that turns the attribute on.
    const fn code(self) -> &'static str {
        match self {
            Self::Bold => "1",
            Self::Dim => "2",
            Self::Italic => "3",
            Self::Underline => "4",
            Self::Blink => "5",
            Self::Inverse => "7",
            Self::Hidden => "8",
            Self::Strikethrough => "9",
        }
    }

    /// Attribute switched on by an SGR parameter.
    const fn from_on_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => Self::Bold,
            2 => Self::Dim,
            3 => Self::Italic,
            4 => Self::Underline,
            5 => Self::Blink,
            7 => Self::Inverse,
            8 => Self::Hidden,
            9 => Self::Strikethrough,
            _ => return None,
        })
    }
}

/// A foreground or background colour as the source spelled it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Color<'a> {
    /// Eight-colour or bright code, written in its canonical decimal form.
    Standard(u32),
    /// Indexed or true-colour parameters, kept verbatim.
    Extended(&'a str),
}

/// Whether an extended colour sets the foreground or the background.
#[derive(Clone, Copy)]
enum Layer {
    /// Text colour.
    Foreground,
    /// Cell background colour.
    Background,
}

/// Terminal style in effect at one point of a text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StyleState<'a> {
    /// Bitmask of active [`Attribute`]s.
    attributes: u8,
    /// Active foreground colour.
    foreground: Option<Color<'a>>,
    /// Active background colour.
    background: Option<Color<'a>>,
    /// Active hyperlink, which a full reset leaves untouched.
    hyperlink: Option<Hyperlink<'a>>,
}

impl<'a> StyleState<'a> {
    /// Applies an OSC 8 escape to the active hyperlink.
    pub(super) fn apply_link(&mut self, change: LinkChange<'a>) {
        self.hyperlink = match change {
            LinkChange::Open(hyperlink) => Some(hyperlink),
            LinkChange::Close => None,
        };
    }

    /// Whether underline is active.
    pub(super) const fn underlined(&self) -> bool {
        self.attributes & Attribute::Underline.bit() != 0
    }

    /// Applies the parameters of an SGR escape.
    pub(super) fn apply_sgr(&mut self, params: &'a str) {
        if params.is_empty() {
            self.reset();
            return;
        }
        let spans = parameter_spans(params);
        let mut index = 0;
        while index < spans.len() {
            let code = params[spans[index].clone()].parse::<u32>().ok();
            if let Some((layer, color, used)) = extended_color(params, &spans, index, code) {
                match layer {
                    Layer::Foreground => self.foreground = Some(color),
                    Layer::Background => self.background = Some(color),
                }
                index += used;
            } else {
                self.apply_code(code);
                index += 1;
            }
        }
    }

    /// Applies one single-parameter SGR code; unknown codes change nothing.
    fn apply_code(&mut self, code: Option<u32>) {
        let Some(code) = code else { return };
        if let Some(attribute) = Attribute::from_on_code(code) {
            self.attributes |= attribute.bit();
            return;
        }
        match code {
            0 => self.reset(),
            21 => self.attributes &= !Attribute::Bold.bit(),
            22 => self.attributes &= !(Attribute::Bold.bit() | Attribute::Dim.bit()),
            23 => self.attributes &= !Attribute::Italic.bit(),
            24 => self.attributes &= !Attribute::Underline.bit(),
            25 => self.attributes &= !Attribute::Blink.bit(),
            27 => self.attributes &= !Attribute::Inverse.bit(),
            28 => self.attributes &= !Attribute::Hidden.bit(),
            29 => self.attributes &= !Attribute::Strikethrough.bit(),
            39 => self.foreground = None,
            49 => self.background = None,
            30..=37 | 90..=97 => self.foreground = Some(Color::Standard(code)),
            40..=47 | 100..=107 => self.background = Some(Color::Standard(code)),
            _ => {}
        }
    }

    /// Clears every attribute and colour; a hyperlink survives.
    fn reset(&mut self) {
        *self = Self {
            hyperlink: self.hyperlink,
            ..Self::default()
        };
    }

    /// Appends the escapes that establish this state from a clean terminal.
    pub(super) fn write_active(&self, out: &mut String) {
        let mut codes = Vec::new();
        for attribute in Attribute::ALL {
            if self.attributes & attribute.bit() != 0 {
                codes.push(attribute.code().to_owned());
            }
        }
        for color in [self.foreground, self.background].into_iter().flatten() {
            codes.push(match color {
                Color::Standard(code) => code.to_string(),
                Color::Extended(spelling) => spelling.to_owned(),
            });
        }
        if !codes.is_empty() {
            out.push_str("\x1b[");
            out.push_str(&codes.join(";"));
            out.push('m');
        }
        if let Some(hyperlink) = self.hyperlink {
            hyperlink.write_open(out);
        }
    }

    /// Appends the underline-off escape when underline is active.
    pub(super) fn write_underline_close(&self, out: &mut String) {
        if self.underlined() {
            out.push_str("\x1b[24m");
        }
    }

    /// Appends the closing escape of the active hyperlink, if any.
    pub(super) fn write_link_close(&self, out: &mut String) {
        if let Some(hyperlink) = self.hyperlink {
            hyperlink.write_close(out);
        }
    }
}

/// Byte ranges of the semicolon-separated parameters.
fn parameter_spans(params: &str) -> Vec<std::ops::Range<usize>> {
    let mut start = 0;
    params
        .split(';')
        .map(|part| {
            let span = start..start + part.len();
            start = span.end + 1;
            span
        })
        .collect()
}

/// Recognizes `38`/`48` followed by indexed (`5;n`) or true-colour (`2;r;g;b`) parameters.
fn extended_color<'a>(
    params: &'a str,
    spans: &[std::ops::Range<usize>],
    index: usize,
    code: Option<u32>,
) -> Option<(Layer, Color<'a>, usize)> {
    let layer = match code? {
        38 => Layer::Foreground,
        48 => Layer::Background,
        _ => return None,
    };
    let part = |offset: usize| spans.get(index + offset).map(|span| &params[span.clone()]);
    let used = match part(1)? {
        "5" => part(2).map(|_| 3)?,
        "2" => part(4).map(|_| 5)?,
        _ => return None,
    };
    let spelling = &params[spans[index].start..spans[index + used - 1].end];
    Some((layer, Color::Extended(spelling), used))
}

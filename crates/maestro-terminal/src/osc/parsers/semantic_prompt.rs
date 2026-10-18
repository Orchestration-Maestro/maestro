//! Semantic prompt actions and independently readable raw options.

use super::super::{Command as OscCommand, Parser};

/// Semantic prompt phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Begin a fresh line.
    FreshLine,
    /// Begin a new prompt on a fresh line.
    FreshLineNewPrompt,
    /// Begin a new command.
    NewCommand,
    /// Begin prompt text.
    PromptStart,
    /// End prompt text and begin input.
    EndPromptStartInput,
    /// Begin input with end-of-line termination.
    EndPromptStartInputTerminateEol,
    /// End input and begin output.
    EndInputStartOutput,
    /// End command execution.
    EndCommand,
}

/// Prompt placement classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// Initial prompt.
    Initial,
    /// Right prompt.
    Right,
    /// Continuation prompt.
    Continuation,
    /// Secondary prompt.
    Secondary,
}

/// Requested prompt redraw policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redraw {
    /// Redraw this prompt.
    True,
    /// Do not redraw this prompt.
    False,
    /// Redraw the last prompt.
    Last,
}

/// Independently selected prompt option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Option {
    /// Prompt placement option.
    PromptKind,
    /// Redraw policy option.
    Redraw,
}

/// Typed prompt option value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionValue {
    /// Prompt placement.
    PromptKind(PromptKind),
    /// Redraw policy.
    Redraw(Redraw),
}

/// An owned prompt action with its unvalidated options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// Prompt phase.
    pub action: Action,
    /// Raw option bytes in input order.
    pub options_unvalidated: Vec<u8>,
}

impl Command {
    /// Construct an action with no options.
    #[must_use]
    pub const fn init(action: Action) -> Self {
        Self {
            action,
            options_unvalidated: Vec::new(),
        }
    }

    /// Read the first matching option without changing its raw bytes.
    #[must_use]
    pub fn read_option(&self, option: Option) -> std::option::Option<OptionValue> {
        option.read(&self.options_unvalidated)
    }
}

impl PromptKind {
    /// Decode a single prompt-kind byte.
    #[must_use]
    pub const fn init(c: u8) -> std::option::Option<Self> {
        match c {
            b'i' => Some(Self::Initial),
            b'r' => Some(Self::Right),
            b'c' => Some(Self::Continuation),
            b's' => Some(Self::Secondary),
            _ => None,
        }
    }
}

impl Option {
    /// Decode the first matching key, including an invalid first match.
    #[must_use]
    pub fn read(self, raw: &[u8]) -> std::option::Option<OptionValue> {
        let key: &[u8] = match self {
            Self::PromptKind => b"k",
            Self::Redraw => b"redraw",
        };
        for field in raw.split(|b| *b == b';') {
            let Some(equal) = field.iter().position(|b| *b == b'=') else {
                continue;
            };
            if &field[..equal] != key {
                continue;
            }
            let value = &field[equal + 1..];
            return match self {
                Self::PromptKind => match value {
                    [c] => PromptKind::init(*c).map(OptionValue::PromptKind),
                    _ => None,
                },
                Self::Redraw => match value {
                    b"0" => Some(OptionValue::Redraw(Redraw::False)),
                    b"1" => Some(OptionValue::Redraw(Redraw::True)),
                    b"last" => Some(OptionValue::Redraw(Redraw::Last)),
                    _ => None,
                },
            };
        }
        None
    }
}

/// Decode one semantic phase and preserve its options verbatim.
pub fn parse(parser: &mut Parser, _: std::option::Option<u8>) -> std::option::Option<OscCommand> {
    let data = parser.data();
    let action = match data.first()? {
        b'L' if data.len() == 1 => Action::FreshLine,
        b'A' => Action::FreshLineNewPrompt,
        b'N' => Action::NewCommand,
        b'P' => Action::PromptStart,
        b'B' => Action::EndPromptStartInput,
        b'I' => Action::EndPromptStartInputTerminateEol,
        b'C' => Action::EndInputStartOutput,
        b'D' => Action::EndCommand,
        _ => return None,
    };
    let options = match data.get(1) {
        None => &[][..],
        Some(b';') => &data[2..],
        _ => return None,
    };
    Some(OscCommand::SemanticPrompt(Command {
        action,
        options_unvalidated: options.to_vec(),
    }))
}

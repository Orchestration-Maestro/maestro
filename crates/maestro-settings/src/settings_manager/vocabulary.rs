//! Finite string vocabularies that retain unrecognized stored values.

use serde_json::Value;

/// Declares a string vocabulary; `Unknown` retains values outside it as written.
macro_rules! vocabulary {
    ($(#[$meta:meta])* $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)+
            /// A value outside the known vocabulary, retained as written.
            Unknown(String),
        }

        impl $name {
            /// The text stored in the settings file.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $text,)+
                    Self::Unknown(text) => text,
                }
            }

            /// Reads stored text, retaining unrecognized values.
            pub(crate) fn from_text(text: &str) -> Self {
                match text {
                    $($text => Self::$variant,)+
                    other => Self::Unknown(other.to_owned()),
                }
            }
        }

        impl From<$name> for Value {
            fn from(value: $name) -> Value {
                match value {
                    $name::Unknown(text) => Value::String(text),
                    known => Value::String(known.as_str().to_owned()),
                }
            }
        }
    };
}

vocabulary! {
    /// The streaming transport preference.
    TransportSetting {
        /// Server-sent events.
        Sse = "sse",
        /// WebSocket.
        Websocket = "websocket",
        /// Choose the transport automatically.
        Auto = "auto",
    }
}

vocabulary! {
    /// How queued steering or follow-up messages are delivered.
    MessageDeliveryMode {
        /// Deliver every queued message together.
        All = "all",
        /// Deliver one queued message at a time.
        OneAtATime = "one-at-a-time",
    }
}

vocabulary! {
    /// The default reasoning effort.
    ThinkingLevel {
        /// No reasoning.
        Off = "off",
        /// Minimal reasoning.
        Minimal = "minimal",
        /// Low reasoning.
        Low = "low",
        /// Medium reasoning.
        Medium = "medium",
        /// High reasoning.
        High = "high",
        /// Extra-high reasoning.
        Xhigh = "xhigh",
    }
}

vocabulary! {
    /// The action of a double escape on an empty editor.
    DoubleEscapeAction {
        /// Fork the conversation.
        Fork = "fork",
        /// Open the conversation tree.
        Tree = "tree",
        /// Do nothing.
        None = "none",
    }
}

/// The filter applied when the conversation tree opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeFilterMode {
    /// Show the default selection of entries.
    Default,
    /// Hide tool entries.
    NoTools,
    /// Show user messages only.
    UserOnly,
    /// Show labeled entries only.
    LabeledOnly,
    /// Show every entry.
    All,
}

impl TreeFilterMode {
    /// The text stored in the settings file.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::NoTools => "no-tools",
            Self::UserOnly => "user-only",
            Self::LabeledOnly => "labeled-only",
            Self::All => "all",
        }
    }

    /// Reads stored text, returning `None` for anything outside the vocabulary.
    pub(crate) fn from_text(text: &str) -> Option<Self> {
        [
            Self::Default,
            Self::NoTools,
            Self::UserOnly,
            Self::LabeledOnly,
            Self::All,
        ]
        .into_iter()
        .find(|mode| mode.as_str() == text)
    }
}

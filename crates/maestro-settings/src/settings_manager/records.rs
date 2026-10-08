//! Sparse stored records and resolved getter results.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::conversion::number;

/// A typed member that a sparse record can read from and write to JSON.
pub(crate) trait Member: Sized {
    /// Reads the member, or `None` when the JSON has another type.
    fn read(value: &Value) -> Option<Self>;
    /// Writes the member as JSON.
    fn write(self) -> Value;
}

impl Member for bool {
    fn read(value: &Value) -> Option<Self> {
        value.as_bool()
    }
    fn write(self) -> Value {
        Value::Bool(self)
    }
}

impl Member for f64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_f64()
    }
    fn write(self) -> Value {
        number(self)
    }
}

impl Member for String {
    fn read(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
    fn write(self) -> Value {
        Value::String(self)
    }
}

/// Conversion between a sparse record and its stored object.
pub(crate) trait Sparse: Default {
    /// Reads a record from a stored object, keeping untyped members.
    fn from_map(map: &Map<String, Value>) -> Self;
    /// Writes typed fields over the retained members, without duplicate keys.
    fn into_map(self) -> Map<String, Value>;
}

/// Declares a sparse record: optional typed fields plus the members that are not
/// typed, including known members whose stored value has another type.
macro_rules! sparse_record {
    ($(#[$meta:meta])* $name:ident { $($(#[$field_meta:meta])* $field:ident: $ty:ty = $key:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: Option<$ty>,)+
            /// Members without a typed field, and known members of another type.
            pub extra: Map<String, Value>,
        }

        impl Sparse for $name {
            fn from_map(map: &Map<String, Value>) -> Self {
                let mut record = Self::default();
                for (key, value) in map {
                    let typed = match key.as_str() {
                        $($key => <$ty as Member>::read(value).map(|read| record.$field = Some(read)),)+
                        _ => None,
                    };
                    if typed.is_none() {
                        record.extra.insert(key.clone(), value.clone());
                    }
                }
                record
            }

            fn into_map(self) -> Map<String, Value> {
                let mut map = Map::new();
                $(if let Some(value) = self.$field {
                    map.insert($key.to_owned(), Member::write(value));
                })+
                for (key, value) in self.extra {
                    map.entry(key).or_insert(value);
                }
                map
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.clone().into_map().serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok(Self::from_map(&Map::deserialize(deserializer)?))
            }
        }
    };
}

impl Member for ProviderRetrySettings {
    fn read(value: &Value) -> Option<Self> {
        value.as_object().map(Self::from_map)
    }
    fn write(self) -> Value {
        Value::Object(self.into_map())
    }
}

sparse_record! {
    /// Context compaction preferences.
    CompactionSettings {
        /// Whether compaction runs automatically.
        enabled: bool = "enabled",
        /// Tokens reserved for the prompt and response.
        reserve_tokens: f64 = "reserveTokens",
        /// Recent tokens kept verbatim.
        keep_recent_tokens: f64 = "keepRecentTokens",
    }
}

sparse_record! {
    /// Branch summarization preferences.
    BranchSummarySettings {
        /// Tokens reserved for the prompt and response.
        reserve_tokens: f64 = "reserveTokens",
        /// Whether the summarize prompt is skipped.
        skip_prompt: bool = "skipPrompt",
    }
}

sparse_record! {
    /// Provider-level request retry preferences.
    ProviderRetrySettings {
        /// Provider request timeout in milliseconds.
        timeout_ms: f64 = "timeoutMs",
        /// Provider retry attempts.
        max_retries: f64 = "maxRetries",
        /// Longest server-requested delay accepted before failing.
        max_retry_delay_ms: f64 = "maxRetryDelayMs",
    }
}

sparse_record! {
    /// Automatic retry preferences.
    RetrySettings {
        /// Whether failed requests are retried.
        enabled: bool = "enabled",
        /// Retry attempts.
        max_retries: f64 = "maxRetries",
        /// Base delay of the exponential backoff in milliseconds.
        base_delay_ms: f64 = "baseDelayMs",
        /// Provider-level retry preferences.
        provider: ProviderRetrySettings = "provider",
    }
}

sparse_record! {
    /// Terminal presentation preferences.
    TerminalSettings {
        /// Whether inline images are shown.
        show_images: bool = "showImages",
        /// Preferred inline image width in terminal cells.
        image_width_cells: f64 = "imageWidthCells",
        /// Whether empty rows are cleared when content shrinks.
        clear_on_shrink: bool = "clearOnShrink",
        /// Whether terminal progress indicators are shown.
        show_terminal_progress: bool = "showTerminalProgress",
    }
}

sparse_record! {
    /// Image handling preferences.
    ImageSettings {
        /// Whether images are resized for model compatibility.
        auto_resize: bool = "autoResize",
        /// Whether images are withheld from model providers.
        block_images: bool = "blockImages",
    }
}

sparse_record! {
    /// Custom token budgets per thinking level.
    ThinkingBudgetsSettings {
        /// Budget for the minimal level.
        minimal: f64 = "minimal",
        /// Budget for the low level.
        low: f64 = "low",
        /// Budget for the medium level.
        medium: f64 = "medium",
        /// Budget for the high level.
        high: f64 = "high",
    }
}

sparse_record! {
    /// Markdown rendering preferences.
    MarkdownSettings {
        /// Indentation of rendered code blocks.
        code_block_indent: String = "codeBlockIndent",
    }
}

sparse_record! {
    /// Warning preferences.
    WarningSettings {
        /// Whether the extra-usage warning is shown.
        anthropic_extra_usage: bool = "anthropicExtraUsage",
    }
}

/// Compaction preferences with every default applied.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCompactionSettings {
    /// Whether compaction runs automatically.
    pub enabled: bool,
    /// Tokens reserved for the prompt and response.
    pub reserve_tokens: f64,
    /// Recent tokens kept verbatim.
    pub keep_recent_tokens: f64,
}

/// Branch summary preferences with every default applied.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedBranchSummarySettings {
    /// Tokens reserved for the prompt and response.
    pub reserve_tokens: f64,
    /// Whether the summarize prompt is skipped.
    pub skip_prompt: bool,
}

/// Retry preferences with every default applied.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRetrySettings {
    /// Whether failed requests are retried.
    pub enabled: bool,
    /// Retry attempts.
    pub max_retries: f64,
    /// Base delay of the exponential backoff in milliseconds.
    pub base_delay_ms: f64,
}

/// Provider retry preferences; only the delay ceiling has a default.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedProviderRetrySettings {
    /// Provider request timeout in milliseconds, when set.
    pub timeout_ms: Option<f64>,
    /// Provider retry attempts, when set.
    pub max_retries: Option<f64>,
    /// Longest server-requested delay accepted before failing.
    pub max_retry_delay_ms: f64,
}

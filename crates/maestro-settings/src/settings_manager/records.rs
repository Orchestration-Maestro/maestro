//! Typed views over stored preference objects and resolved getter results.

use serde_json::{Map, Value};

use super::conversion::number;

/// A typed member that an object view can read from and write to JSON.
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

impl Member for ProviderRetrySettings {
    fn read(value: &Value) -> Option<Self> {
        value.as_object().cloned().map(Self)
    }
    fn write(self) -> Value {
        Value::Object(self.0)
    }
}

/// Reads a typed member of a stored object.
pub(crate) fn get_member<T: Member>(object: &Map<String, Value>, key: &str) -> Option<T> {
    object.get(key).and_then(T::read)
}

/// Replaces a member in place, appends it when new, or removes it for `None`.
pub(crate) fn set_member<T: Member>(object: &mut Map<String, Value>, key: &str, value: Option<T>) {
    match value {
        Some(value) => {
            object.insert(key.to_owned(), value.write());
        }
        None => {
            object.shift_remove(key);
        }
    }
}

/// Declares a typed view over a stored object. The view owns the object as
/// written: a getter reads one typed member, a setter replaces a member where it
/// stands, appends a new member at the end or removes it, and every other member
/// keeps its value and position.
macro_rules! object_view {
    (
        $(#[$meta:meta])* $name:ident {
            $($(#[$field_meta:meta])* $get:ident / $set:ident: $ty:ty = $key:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Default, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(
            /// The stored object, with members of any type in their original order.
            pub ::serde_json::Map<String, ::serde_json::Value>,
        );

        impl $name {
            $(
                $(#[$field_meta])*
                #[must_use]
                pub fn $get(&self) -> Option<$ty> {
                    $crate::settings_manager::records::get_member(&self.0, $key)
                }

                #[doc = concat!("Sets `", $key, "` in place, appending it when new; `None` removes it.")]
                pub fn $set(&mut self, value: Option<$ty>) {
                    $crate::settings_manager::records::set_member(&mut self.0, $key, value);
                }
            )+
        }
    };
}
pub(super) use object_view;

object_view! {
    /// Context compaction preferences.
    CompactionSettings {
        /// Whether compaction runs automatically.
        enabled / set_enabled: bool = "enabled",
        /// Tokens reserved for the prompt and response.
        reserve_tokens / set_reserve_tokens: f64 = "reserveTokens",
        /// Recent tokens kept verbatim.
        keep_recent_tokens / set_keep_recent_tokens: f64 = "keepRecentTokens",
    }
}

object_view! {
    /// Branch summarization preferences.
    BranchSummarySettings {
        /// Tokens reserved for the prompt and response.
        reserve_tokens / set_reserve_tokens: f64 = "reserveTokens",
        /// Whether the summarize prompt is skipped.
        skip_prompt / set_skip_prompt: bool = "skipPrompt",
    }
}

object_view! {
    /// Provider-level request retry preferences.
    ProviderRetrySettings {
        /// Provider request timeout in milliseconds.
        timeout_ms / set_timeout_ms: f64 = "timeoutMs",
        /// Provider retry attempts.
        max_retries / set_max_retries: f64 = "maxRetries",
        /// Longest server-requested delay accepted before failing.
        max_retry_delay_ms / set_max_retry_delay_ms: f64 = "maxRetryDelayMs",
    }
}

object_view! {
    /// Automatic retry preferences.
    RetrySettings {
        /// Whether failed requests are retried.
        enabled / set_enabled: bool = "enabled",
        /// Retry attempts.
        max_retries / set_max_retries: f64 = "maxRetries",
        /// Base delay of the exponential backoff in milliseconds.
        base_delay_ms / set_base_delay_ms: f64 = "baseDelayMs",
        /// Provider-level retry preferences.
        provider / set_provider: ProviderRetrySettings = "provider",
    }
}

object_view! {
    /// Terminal presentation preferences.
    TerminalSettings {
        /// Whether inline images are shown.
        show_images / set_show_images: bool = "showImages",
        /// Preferred inline image width in terminal cells.
        image_width_cells / set_image_width_cells: f64 = "imageWidthCells",
        /// Whether empty rows are cleared when content shrinks.
        clear_on_shrink / set_clear_on_shrink: bool = "clearOnShrink",
        /// Whether terminal progress indicators are shown.
        show_terminal_progress / set_show_terminal_progress: bool = "showTerminalProgress",
    }
}

object_view! {
    /// Image handling preferences.
    ImageSettings {
        /// Whether images are resized for model compatibility.
        auto_resize / set_auto_resize: bool = "autoResize",
        /// Whether images are withheld from model providers.
        block_images / set_block_images: bool = "blockImages",
    }
}

object_view! {
    /// Custom token budgets per thinking level.
    ThinkingBudgetsSettings {
        /// Budget for the minimal level.
        minimal / set_minimal: f64 = "minimal",
        /// Budget for the low level.
        low / set_low: f64 = "low",
        /// Budget for the medium level.
        medium / set_medium: f64 = "medium",
        /// Budget for the high level.
        high / set_high: f64 = "high",
    }
}

object_view! {
    /// Markdown rendering preferences.
    MarkdownSettings {
        /// Indentation of rendered code blocks.
        code_block_indent / set_code_block_indent: String = "codeBlockIndent",
    }
}

object_view! {
    /// Warning preferences.
    WarningSettings {
        /// Whether the extra-usage warning is shown.
        anthropic_extra_usage / set_anthropic_extra_usage: bool = "anthropicExtraUsage",
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

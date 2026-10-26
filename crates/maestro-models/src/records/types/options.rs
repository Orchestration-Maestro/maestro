//! Invocation settings with retained adapter hooks.
use super::Threaded;
use crate::providers::http::Fetch;
use serde::de::IntoDeserializer;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
/// Common invocation settings retaining the selected execution family.
///
/// ```
/// use maestro_models::StreamOptions;
/// use std::sync::Arc;
/// let options: StreamOptions = StreamOptions {
///     on_payload: Some(Arc::new(|input, _| Box::pin(async move { Ok(input) }))),
///     on_response: Some(Arc::new(|_, _| Box::pin(async { Ok(()) }))),
///     ..Default::default()
/// };
/// assert!(options.on_payload.is_some() && options.on_response.is_some());
/// ```
pub type StreamOptions<E = Threaded> = maestro_request::types::StreamOptions<E, Fetch>;
/// Object handle retained by [`ProviderObjects`].
#[cfg(not(target_arch = "wasm32"))]
type ObjectHandle = std::sync::Arc<dyn std::any::Any + Send + Sync>;
/// Browser-local object handle retained by [`ProviderObjects`].
#[cfg(target_arch = "wasm32")]
type ObjectHandle = std::sync::Arc<dyn std::any::Any>;

/// Typed, non-serializable objects handed to a provider, at most one per concrete type.
///
/// Cloning copies the container and shares the stored objects; replacing an entry in one
/// container never changes another.
#[derive(Clone, Default)]
pub struct ProviderObjects {
    /// Retained handles keyed by their concrete type.
    entries: std::collections::HashMap<std::any::TypeId, ObjectHandle>,
}

/// Generate typed insertion with the target's object-sharing bounds.
macro_rules! provider_object_insert {
    ($($bound:tt)+) => {
        /// Store `value`, replacing any earlier value of the same type.
        pub fn insert<T: $($bound)+>(&mut self, value: T) {
            self.entries
                .insert(std::any::TypeId::of::<T>(), std::sync::Arc::new(value));
        }
    };
}

impl ProviderObjects {
    #[cfg(not(target_arch = "wasm32"))]
    provider_object_insert!(std::any::Any + Send + Sync);
    #[cfg(target_arch = "wasm32")]
    provider_object_insert!(std::any::Any);

    /// Borrow the stored value of type `T`, if any.
    #[must_use]
    pub fn get<T: std::any::Any>(&self) -> Option<&T> {
        self.entries
            .get(&std::any::TypeId::of::<T>())?
            .downcast_ref()
    }
}

impl std::fmt::Debug for ProviderObjects {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderObjects")
            .field("entries", &self.entries.len())
            .finish()
    }
}

/// Original common and open options with native opaque objects.
pub type ProviderStreamOptions<E = Threaded> =
    maestro_request::types::ProviderStreamOptions<StreamOptions<E>, ProviderObjects>;
/// Common invocation settings with native tool selection.
pub type SimpleStreamOptions<E = Threaded> =
    maestro_request::types::SimpleStreamOptions<StreamOptions<E>, ToolChoice>;

/// Original callback options with native execution-family defaults and opaque controls.
pub type StreamOptionsInput<E = Threaded> =
    maestro_request::types::StreamOptionsInput<StreamOptions<E>, ProviderObjects, ToolChoice>;

/// Select whether and which tool the model must call.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "ToolChoiceWire")]
pub enum ToolChoice {
    /// Let the model decide.
    Auto,
    /// Forbid tool calls.
    None,
    /// Require some tool call.
    Required,
    /// Require a call to the named function.
    Function {
        /// Function name.
        name: String,
    },
}

/// Serialized tool choice: a mode string or a named function object.
#[derive(Deserialize)]
#[serde(untagged)]
enum ToolChoiceWire {
    /// Mode string.
    Mode(Literal<ToolChoiceMode>),
    /// Named function object.
    Function {
        /// Constant discriminator.
        r#type: Literal<FunctionTag>,
        /// Function reference.
        function: FunctionName,
    },
}

/// Mode strings accepted for a tool choice.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ToolChoiceMode {
    /// Let the model decide.
    Auto,
    /// Forbid tool calls.
    None,
    /// Require some tool call.
    Required,
}

/// Constant `function` discriminator.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FunctionTag {
    /// The only accepted discriminator.
    Function,
}

/// An enum read only from a JSON string; maps such as `{"high": null}` are rejected.
pub(crate) struct Literal<T>(pub(crate) T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Literal<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        T::deserialize(text.into_deserializer()).map(Self)
    }
}

/// Function reference inside a named tool choice, read only from a JSON object.
pub(crate) struct FunctionName {
    /// Function name.
    pub(crate) name: String,
}

impl<'de> Deserialize<'de> for FunctionName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Fields {
            name: String,
        }
        let fields = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        Fields::deserialize(serde_json::Value::Object(fields))
            .map(|Fields { name }| Self { name })
            .map_err(serde::de::Error::custom)
    }
}

/// Borrowed function reference written inside a named tool choice.
#[derive(Serialize)]
struct FunctionNameRef<'a> {
    /// Function name.
    name: &'a str,
}

impl From<ToolChoiceWire> for ToolChoice {
    fn from(wire: ToolChoiceWire) -> Self {
        match wire {
            ToolChoiceWire::Mode(Literal(ToolChoiceMode::Auto)) => Self::Auto,
            ToolChoiceWire::Mode(Literal(ToolChoiceMode::None)) => Self::None,
            ToolChoiceWire::Mode(Literal(ToolChoiceMode::Required)) => Self::Required,
            ToolChoiceWire::Function {
                r#type: Literal(FunctionTag::Function),
                function,
            } => Self::Function {
                name: function.name,
            },
        }
    }
}

impl Serialize for ToolChoice {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::None => serializer.serialize_str("none"),
            Self::Required => serializer.serialize_str("required"),
            Self::Function { name } => {
                let mut choice = serializer.serialize_struct("ToolChoice", 2)?;
                choice.serialize_field("type", "function")?;
                choice.serialize_field("function", &FunctionNameRef { name })?;
                choice.end()
            }
        }
    }
}

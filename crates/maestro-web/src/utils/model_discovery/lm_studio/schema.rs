//! Downloaded-record schema admission before filtering and projection.
use super::super::{Members, ModelNames, field, members, model};
use maestro_models::{Model, ModelInput};
use serde::de::Error as _;
use serde_json::value::RawValue;

/// A validated language-model descriptor without unused server metadata.
pub(super) struct Downloaded {
    /// Final invocation path and display name.
    names: ModelNames,
    /// Context and output token limit.
    context: f64,
    /// Image-input capability.
    vision: bool,
    /// Tool-training capability used as reasoning support.
    reasoning: bool,
}
impl Downloaded {
    /// Project the admitted record into the shared model descriptor.
    pub(super) fn model(self, base: &str) -> Model {
        let mut result = model(self.names, base, self.context, self.context, self.reasoning);
        if self.vision {
            result.input.push(ModelInput::Image);
        }
        result
    }
}
/// Reject a selected field outside its wire domain.
pub(super) fn invalid(message: &str) -> serde_json::Error {
    serde_json::Error::custom(message)
}
/// Read a finite integral double without an integer-width restriction.
pub(super) fn integer(object: &Members, key: &str) -> Result<f64, serde_json::Error> {
    let number: f64 = field(object, key)?;
    if number.is_finite() && number.fract() == 0.0 {
        Ok(number)
    } else {
        Err(invalid("Expected an integer"))
    }
}
/// Validate unused optional metadata without conflating null and absence.
fn metadata(object: &Members) -> Result<(), serde_json::Error> {
    for key in ["paramsString", "architecture"] {
        if object.contains_key(key) {
            field::<String>(object, key)?;
        }
    }
    if let Some(value) = object.get("quantization") {
        let quantization = members(value.get())?;
        field::<String>(&quantization, "name")?;
        integer(&quantization, "bits")?;
    }
    Ok(())
}
/// Admit the entire record, including fields on filtered embeddings.
fn record(raw: &RawValue) -> Result<Option<Downloaded>, serde_json::Error> {
    let object = members(raw.get())?;
    let kind: String = field(&object, "type")?;
    if !matches!(kind.as_str(), "llm" | "embedding") {
        return Err(invalid("Unknown model type"));
    }
    field::<String>(&object, "modelKey")?;
    let format: String = field(&object, "format")?;
    if !matches!(
        format.as_str(),
        "gguf" | "safetensors" | "onnx" | "ggml" | "mlx_placeholder" | "torch_safetensors"
    ) {
        return Err(invalid("Unknown model format"));
    }
    let name: String = field(&object, "displayName")?;
    let id: String = field(&object, "path")?;
    integer(&object, "sizeBytes")?;
    let context = integer(&object, "maxContextLength")?;
    metadata(&object)?;
    if kind == "embedding" {
        return Ok(None);
    }
    Ok(Some(Downloaded {
        names: if name.is_empty() {
            ModelNames::same(id)
        } else {
            ModelNames { id, name }
        },
        context,
        vision: field(&object, "vision")?,
        reasoning: field(&object, "trainedForToolUse")?,
    }))
}
/// Validate every array element before returning selected language records.
pub(super) fn listing(raw: &str) -> Result<Vec<Downloaded>, serde_json::Error> {
    let rows: Vec<Box<RawValue>> = serde_json::from_str(raw)?;
    rows.iter()
        .map(|raw| record(raw))
        .collect::<Result<Vec<_>, _>>()
        .map(|rows| rows.into_iter().flatten().collect())
}

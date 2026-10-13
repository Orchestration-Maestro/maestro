//! Generation settings and body-level request members of the public endpoint.

use serde_json::{Map, Value, json};

use super::{
    Failure, Projection, Step, content, record, record_list, schema, select, tools, type_name,
};

/// Selection of the configuration in wire order; body-level members go to the request body
/// and the rest to the generation object.
const CONFIG: &[Step] = &[
    Step::Body("systemInstruction", content::system_instruction),
    Step::Keep("temperature"),
    Step::Keep("topP"),
    Step::Keep("topK"),
    Step::Keep("candidateCount"),
    Step::Keep("maxOutputTokens"),
    Step::Keep("stopSequences"),
    Step::Keep("responseLogprobs"),
    Step::Keep("logprobs"),
    Step::Keep("presencePenalty"),
    Step::Keep("frequencyPenalty"),
    Step::Keep("seed"),
    Step::Keep("responseMimeType"),
    Step::Convert("responseSchema", schema::convert),
    Step::Keep("responseJsonSchema"),
    Step::Reject("routingConfig"),
    Step::Reject("modelSelectionConfig"),
    Step::Body("safetySettings", |settings| record_list(settings, SAFETY)),
    Step::Body("tools", tools::tools),
    Step::Body("toolConfig", tools::tool_config),
    Step::Reject("labels"),
    Step::Body("cachedContent", cached_content),
    Step::Keep("responseModalities"),
    Step::Keep("mediaResolution"),
    Step::Convert("speechConfig", speech),
    Step::Reject("audioTimestamp"),
    Step::Keep("thinkingConfig"),
    Step::Convert("imageConfig", |image| record(image, IMAGE)),
    Step::Keep("enableEnhancedCivicAnswers"),
    Step::Reject("modelArmorConfig"),
    Step::Body("serviceTier", Ok),
];

/// Selection of image settings.
const IMAGE: &[Step] = &[
    Step::Keep("aspectRatio"),
    Step::Keep("imageSize"),
    Step::Reject("personGeneration"),
    Step::Reject("prominentPeople"),
    Step::Reject("outputMimeType"),
    Step::Reject("outputCompressionQuality"),
    Step::Reject("imageOutputOptions"),
];

/// Selection of one safety setting.
const SAFETY: &[Step] = &[
    Step::Keep("category"),
    Step::Reject("method"),
    Step::Keep("threshold"),
];

/// Select the generation object of a configuration, adding its body-level members to `body`.
///
/// Routes the response schema through the schema owner's JSON-alternative rule first.
pub(super) fn generation(
    mut config: Value,
    body: &mut Map<String, Value>,
) -> Projection<Map<String, Value>> {
    if let Value::Object(members) = &mut config {
        schema::route_json_schema(members, "responseSchema", "responseJsonSchema");
    }
    select(config, CONFIG, body)
}

/// A cached-content name; a single unqualified segment gains its resource prefix.
fn cached_content(name: Value) -> Projection<Value> {
    match name {
        Value::String(name) if !name.starts_with("cachedContents/") && !name.contains('/') => {
            Ok(Value::String(format!("cachedContents/{name}")))
        }
        Value::String(_) => Ok(name),
        _ => Err(Failure::error("name must be a string")),
    }
}

/// A speech configuration: records are kept whole and a voice name becomes a prebuilt voice.
fn speech(speech: Value) -> Projection<Value> {
    match speech {
        Value::Object(_) | Value::Array(_) => Ok(speech),
        Value::String(voice) => {
            Ok(json!({"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": voice}}}))
        }
        other => Err(Failure::error(format!(
            "Unsupported speechConfig type: {}",
            type_name(&other)
        ))),
    }
}

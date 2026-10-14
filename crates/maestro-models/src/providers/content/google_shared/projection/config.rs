//! Generation settings and body-level request members of the selected endpoint.

use serde_json::{Map, Value, json};

use super::{
    Endpoint, Failure, Projection, Step, content, record, record_list, schema, select, tools,
    type_name,
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
    Step::Convert("responseSchema", |value, _| schema::convert(value)),
    Step::Keep("responseJsonSchema"),
    Step::Reject("routingConfig"),
    Step::Reject("modelSelectionConfig"),
    Step::Body("safetySettings", |settings, endpoint| {
        record_list(settings, SAFETY, endpoint)
    }),
    Step::Body("tools", tools::tools),
    Step::Body("toolConfig", tools::tool_config),
    Step::Reject("labels"),
    Step::Body("cachedContent", cached_content),
    Step::Keep("responseModalities"),
    Step::Keep("mediaResolution"),
    Step::Convert("speechConfig", speech),
    Step::Reject("audioTimestamp"),
    Step::Keep("thinkingConfig"),
    Step::Convert("imageConfig", |image, endpoint| {
        record(image, IMAGE, endpoint)
    }),
    Step::Keep("enableEnhancedCivicAnswers"),
    Step::Reject("modelArmorConfig"),
    Step::Body("serviceTier", |value, _| Ok(value)),
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
    endpoint: Endpoint<'_>,
) -> Projection<Map<String, Value>> {
    if let Value::Object(members) = &mut config {
        schema::route_json_schema(members, "responseSchema", "responseJsonSchema");
    }
    let plan = match endpoint {
        Endpoint::Public => CONFIG,
        Endpoint::Vertex { .. } => VERTEX_CONFIG,
    };
    select(config, plan, body, endpoint)
}

/// A cached-content name; a single unqualified segment gains its resource prefix.
fn cached_content(name: Value, endpoint: Endpoint<'_>) -> Projection<Value> {
    let Value::String(name) = name else {
        return Err(Failure::error("name must be a string"));
    };
    let qualified = match endpoint {
        Endpoint::Public if !name.starts_with("cachedContents/") && !name.contains('/') => {
            format!("cachedContents/{name}")
        }
        Endpoint::Vertex { project, location } if !name.starts_with("projects/") => {
            let project = project.unwrap_or("undefined");
            let location = location.unwrap_or("undefined");
            if name.starts_with("locations/") {
                format!("projects/{project}/{name}")
            } else if name.starts_with("cachedContents/") {
                format!("projects/{project}/locations/{location}/{name}")
            } else if !name.contains('/') {
                format!("projects/{project}/locations/{location}/cachedContents/{name}")
            } else {
                name
            }
        }
        _ => name,
    };
    Ok(Value::String(qualified))
}

/// A speech configuration: records are kept whole and a voice name becomes a prebuilt voice.
fn speech(speech: Value, _endpoint: Endpoint<'_>) -> Projection<Value> {
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

/// Cloud configuration selection in wire order.
const VERTEX_CONFIG: &[Step] = &[
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
    Step::Convert("responseSchema", |value, _| schema::convert(value)),
    Step::Keep("responseJsonSchema"),
    Step::Keep("routingConfig"),
    Step::Rename("modelSelectionConfig", "modelConfig"),
    Step::Body("safetySettings", |value, _| Ok(value)),
    Step::Body("tools", tools::tools),
    Step::Body("toolConfig", tools::tool_config),
    Step::Body("labels", |value, _| Ok(value)),
    Step::Body("cachedContent", cached_content),
    Step::Keep("responseModalities"),
    Step::Keep("mediaResolution"),
    Step::Convert("speechConfig", speech),
    Step::Keep("audioTimestamp"),
    Step::Keep("thinkingConfig"),
    Step::Convert("imageConfig", vertex_image),
    Step::Reject("enableEnhancedCivicAnswers"),
    Step::Body("modelArmorConfig", |value, _| Ok(value)),
    Step::Body("serviceTier", |value, _| Ok(value)),
];

/// Cloud image settings with output aliases grouped into their wire record.
fn vertex_image(image: Value, endpoint: Endpoint<'_>) -> Projection<Value> {
    let mut image = match image {
        Value::Object(image) => image,
        _ => Map::new(),
    };
    let mut output = Map::new();
    for (source, target) in [
        ("outputMimeType", "mimeType"),
        ("outputCompressionQuality", "compressionQuality"),
    ] {
        if let Some(value) = super::take(&mut image, source) {
            output.insert(target.to_owned(), value);
        }
    }
    let explicit = super::take(&mut image, "imageOutputOptions");
    let mut selected = select(
        Value::Object(image),
        VERTEX_IMAGE,
        &mut Map::new(),
        endpoint,
    )?;
    let options = match explicit {
        Some(Value::Object(members)) => {
            output.extend(members);
            Some(Value::Object(output))
        }
        Some(other) => Some(other),
        None if !output.is_empty() => Some(Value::Object(output)),
        None => None,
    };
    if let Some(options) = options {
        selected.insert("imageOutputOptions".to_owned(), options);
    }
    Ok(Value::Object(selected))
}

/// Cloud image members preceding the grouped output options.
const VERTEX_IMAGE: &[Step] = &[
    Step::Keep("aspectRatio"),
    Step::Keep("imageSize"),
    Step::Keep("personGeneration"),
    Step::Keep("prominentPeople"),
];

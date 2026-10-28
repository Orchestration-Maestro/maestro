//! Original-document checking before typed model selection.
use super::{
    ModelFileOperations,
    composition::{self, ProviderConfig},
};
use maestro_models::{ModelCompat, arguments::validation::validate_schema};
use serde_json::Value;
use std::sync::LazyLock;

/// Source-derived file schema, retained without conversion.
static SCHEMA: LazyLock<Result<Value, serde_json::Error>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../../assets/models.schema.json")));
/// Read checked documents in order, retaining undefined fields and replacing custom models.
pub(super) fn load(
    paths: &[String],
    operations: &dyn ModelFileOperations,
) -> Result<Option<Vec<(String, ProviderConfig)>>, String> {
    let mut combined = None;
    let mut sources = Vec::new();
    for path in paths {
        if let Some(value) = read_document(path, operations)? {
            sources.push(path.as_str());
            match &mut combined {
                Some(current) => merge_document(current, value, 0),
                None => combined = Some(value),
            }
        }
    }
    let Some(value) = combined else {
        return Ok(None);
    };
    let sources = sources.join(", ");
    let providers = composition::providers(value)
        .map_err(|error| envelope("Failed to load models.json", &error.to_string(), &sources))?;
    validate(&providers)
        .map_err(|error| envelope("Failed to load models.json", &error, &sources))?;
    Ok(Some(providers))
}

/// Checks each original document before any merging; missing files contribute nothing.
fn read_document(
    path: &str,
    operations: &dyn ModelFileOperations,
) -> Result<Option<Value>, String> {
    if path.is_empty() || !operations.exists(path) {
        return Ok(None);
    }
    let text = operations
        .read_to_string(path)
        .map_err(|e| envelope("Failed to load models.json", &e.to_string(), path))?;
    let value = serde_json::from_str(&strip_comments(&text))
        .map_err(|e| envelope("Failed to parse models.json", &e.to_string(), path))?;
    let schema = SCHEMA
        .as_ref()
        .map_err(|e| envelope("Failed to load models.json", &e.to_string(), path))?;
    let errors = validate_schema(schema, &value)
        .map_err(|e| envelope("Failed to load models.json", &e.message, path))?;
    if !errors.is_empty() {
        return Err(envelope(
            "Invalid models.json schema",
            &errors.join("\n"),
            path,
        ));
    }
    Ok(Some(value))
}

/// Merges fields recursively; only `providers.<provider>.models` at depth two uses identities.
fn merge_document(current: &mut Value, incoming: Value, depth: usize) {
    match (current, incoming) {
        (Value::Object(current), Value::Object(incoming)) => {
            for (key, value) in incoming {
                match current.get_mut(&key) {
                    Some(existing) if depth == 2 && key == "models" => {
                        merge_models(existing, value);
                    }
                    Some(existing) if matches!(depth, 2 | 4) && key == "compat" => {
                        merge_compat_fields(existing, value);
                    }
                    Some(existing) => merge_document(existing, value, depth + 1),
                    None => {
                        current.insert(key, value);
                    }
                }
            }
        }
        (current, incoming) => *current = incoming,
    }
}

/// Compatibility options use the same routing overlays as descriptor composition.
fn merge_compat_fields(current: &mut Value, incoming: Value) {
    let compat = |value| match value {
        Value::Object(fields) => Some(ModelCompat(fields)),
        _ => None,
    };
    *current = composition::merge_compat(compat(current.take()), compat(incoming))
        .map_or(Value::Null, |value| Value::Object(value.0));
}

/// Later custom definitions replace the whole model without moving its first list slot.
fn merge_models(current: &mut Value, incoming: Value) {
    match (current, incoming) {
        (Value::Array(current), Value::Array(incoming)) => {
            for model in incoming {
                if let Some(position) = current
                    .iter()
                    .position(|existing| existing.get("id") == model.get("id"))
                {
                    current.retain(|existing| existing.get("id") != model.get("id"));
                    current.insert(position, model);
                } else {
                    current.push(model);
                }
            }
        }
        (current, incoming) => *current = incoming,
    }
}

/// Remove line comments first, then recognize trailing commas on the resulting text.
fn strip_comments(text: &str) -> String {
    let mut without_comments = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' => quoted(&mut characters, &mut without_comments),
            '/' if characters.peek() == Some(&'/') => {
                if characters.by_ref().any(|next| next == '\n') {
                    without_comments.push('\n');
                }
            }
            _ => without_comments.push(character),
        }
    }
    let mut output = String::with_capacity(without_comments.len());
    let mut characters = without_comments.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' => quoted(&mut characters, &mut output),
            ',' => {
                let next = characters.clone().find(|c| !comma_whitespace(*c));
                if !matches!(next, Some('}' | ']')) {
                    output.push(character);
                }
            }
            _ => output.push(character),
        }
    }
    output
}
/// Copy a quoted token with its escapes untouched.
fn quoted(characters: &mut std::iter::Peekable<std::str::Chars<'_>>, output: &mut String) {
    output.push('"');
    while let Some(character) = characters.next() {
        output.push(character);
        match character {
            '"' => break,
            '\\' => {
                if let Some(escaped) = characters.next() {
                    output.push(escaped);
                }
            }
            _ => {}
        }
    }
}
use maestro_text::is_whitespace_scalar as comma_whitespace;
/// Enforce custom-model prerequisites in source provider and model order.
fn validate(providers: &[(String, ProviderConfig)]) -> Result<(), String> {
    let builtins: std::collections::HashSet<_> =
        maestro_models::get_providers().into_iter().collect();
    for (name, config) in providers {
        validate_provider(name, config, builtins.contains(name))?;
    }
    Ok(())
}
/// Check one provider before checking each of its model limits.
fn validate_provider(name: &str, config: &ProviderConfig, builtin: bool) -> Result<(), String> {
    if config.models.is_empty() {
        if config.base_url.is_none()
            && config.headers.is_none()
            && config.compat.is_none()
            && config.model_overrides.is_empty()
        {
            return Err(format!(
                "Provider {name}: must specify \"baseUrl\", \"headers\", \"compat\", \"modelOverrides\", or \"models\"."
            ));
        }
    } else if !builtin {
        for (field, present) in [
            ("baseUrl", config.base_url.is_some()),
            ("apiKey", config.api_key.is_some()),
        ] {
            if !present {
                return Err(format!(
                    "Provider {name}: \"{field}\" is required when defining custom models."
                ));
            }
        }
    }
    for model in &config.models {
        if !builtin && config.api.is_none() && model.api.is_none() {
            return Err(format!(
                "Provider {name}, model {}: no \"api\" specified. Set at provider or model level.",
                model.id
            ));
        }
        for (field, value) in [
            ("contextWindow", model.context_window),
            ("maxTokens", model.max_tokens),
        ] {
            if value.is_some_and(|n| n <= 0.0) {
                return Err(format!(
                    "Provider {name}, model {}: invalid {field}",
                    model.id
                ));
            }
        }
    }
    Ok(())
}
/// Format a load phase with its explicit path.
fn envelope(prefix: &str, message: &str, path: &str) -> String {
    let separator = if prefix == "Invalid models.json schema" {
        "\n"
    } else {
        " "
    };
    format!("{prefix}:{separator}{message}\n\nFile: {path}")
}
#[cfg(test)]
mod tests;

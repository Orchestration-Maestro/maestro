//! Single-fetch acquisition and source-specific progress reporting.
use super::input::{self, Invalid};
use crate::providers::json_text::{member, raw_json};
use crate::{Fetch, HttpRequest, Model, ModelInput};
use futures_util::StreamExt;
use serde_json::value::RawValue;
use std::io::{self, Write};

/// Retrieve `OpenRouter`'s tool-capable model descriptors.
///
/// # Errors
/// Returns an output or diagnostic writer failure. Source failures return an empty
/// vector if writing the diagnostic succeeds; malformed selected records are
/// skipped individually if writing their diagnostics succeeds.
pub async fn fetch_open_router_models(
    fetch: &Fetch,
    output: &mut dyn Write,
    errors: &mut dyn Write,
) -> io::Result<Vec<Model>> {
    retrieve(fetch, output, errors, Source::Router).await
}
/// Retrieve Vercel AI Gateway's tool-capable model descriptors.
///
/// # Errors
/// Returns an output or diagnostic writer failure. Source failures return an empty
/// vector if writing the diagnostic succeeds; malformed selected records are
/// skipped individually if writing their diagnostics succeeds.
pub async fn fetch_ai_gateway_models(
    fetch: &Fetch,
    output: &mut dyn Write,
    errors: &mut dyn Write,
) -> io::Result<Vec<Model>> {
    retrieve(fetch, output, errors, Source::Gateway).await
}
/// Retrieve models.dev's routed tool-capable model descriptors.
///
/// # Errors
/// Returns an output or diagnostic writer failure. Source failures return an empty
/// vector if writing the diagnostic succeeds; malformed selected records are
/// skipped individually if writing their diagnostics succeeds.
pub async fn load_models_dev_data(
    fetch: &Fetch,
    output: &mut dyn Write,
    errors: &mut dyn Write,
) -> io::Result<Vec<Model>> {
    retrieve(fetch, output, errors, Source::Dev).await
}
/// Selected external catalog source.
#[derive(Clone, Copy)]
enum Source {
    /// `OpenRouter` catalog.
    Router,
    /// Vercel catalog.
    Gateway,
    /// Provider metadata catalog.
    Dev,
}
impl Source {
    /// Human-readable source label.
    fn label(self) -> &'static str {
        match self {
            Self::Router => "OpenRouter",
            Self::Gateway => "Vercel AI Gateway",
            Self::Dev => "models.dev",
        }
    }
    /// Authored endpoint.
    fn url(self) -> &'static str {
        match self {
            Self::Router => "https://openrouter.ai/api/v1/models",
            Self::Gateway => "https://ai-gateway.vercel.sh/v1/models",
            Self::Dev => "https://models.dev/api.json",
        }
    }
    /// Application-owned failure prefix.
    fn prefix(self) -> &'static str {
        match self {
            Self::Router => "Failed to fetch OpenRouter models:",
            Self::Gateway => "Failed to fetch Vercel AI Gateway models:",
            Self::Dev => "Failed to load models.dev data:",
        }
    }
}
/// Fetch the complete body without status policy or retries.
async fn body(fetch: &Fetch, source: Source) -> Result<String, String> {
    let request = HttpRequest {
        method: "GET".into(),
        url: source.url().into(),
        headers: indexmap::IndexMap::new(),
        body: Vec::new(),
        signal: None,
    };
    let mut response = fetch(request).await.map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.body.next().await {
        bytes.extend(chunk.map_err(|error| error.to_string())?);
    }
    Ok(crate::providers::http::decode_utf8(&bytes).into_owned())
}
/// Report acquisition and its completed count.
async fn retrieve(
    fetch: &Fetch,
    output: &mut dyn Write,
    errors: &mut dyn Write,
    source: Source,
) -> io::Result<Vec<Model>> {
    writeln!(output, "Fetching models from {} API...", source.label())?;
    let text = match body(fetch, source).await {
        Ok(text) => text,
        Err(error) => {
            writeln!(errors, "{} {error}", source.prefix())?;
            return Ok(Vec::new());
        }
    };
    let projected_text = input::string_units(&text);
    let raw = match raw_json(&projected_text) {
        Ok(raw) => raw,
        Err(error) => {
            writeln!(errors, "{} {error}", source.prefix())?;
            return Ok(Vec::new());
        }
    };
    if raw.get() == "null"
        || (matches!(source, Source::Router)
            && !member(raw, "data")
                .is_some_and(|raw| raw.get().starts_with('[') || raw.get().starts_with('"')))
    {
        writeln!(errors, "{} invalid feed collection", source.prefix())?;
        return Ok(Vec::new());
    }
    let models = normalize(raw, source, errors, &text, &projected_text)?;
    let verb = if matches!(source, Source::Dev) {
        "Loaded"
    } else {
        "Fetched"
    };
    writeln!(
        output,
        "{verb} {} tool-capable models from {}",
        models.len(),
        source.label()
    )?;
    Ok(models)
}

/// Normalize the selected feed records.
fn normalize(
    raw: &RawValue,
    source: Source,
    errors: &mut dyn Write,
    original_text: &str,
    projected_text: &str,
) -> io::Result<Vec<Model>> {
    let mut models = Vec::new();
    match source {
        Source::Router | Source::Gateway => {
            let items: Vec<&RawValue> = member(raw, "data")
                .and_then(|data| serde_json::from_str(data.get()).ok())
                .unwrap_or_default();
            for (index, item) in items.into_iter().enumerate() {
                match gateway_model(item, source) {
                    Ok(Some(model)) => models.push(model),
                    Ok(None) => {}
                    Err(_) => {
                        let id = match member(item, "id") {
                            Some(id) => diagnostic_id(id, original_text, projected_text)?,
                            None => serde_json::to_string(&format!("<entry:{index}>"))?,
                        };
                        writeln!(
                            errors,
                            "{} skipping model {id}: invalid metadata",
                            source.prefix()
                        )?;
                    }
                }
            }
        }
        Source::Dev => return super::models_dev::normalize(raw, errors),
    }
    Ok(models)
}
/// Project one gateway record after its capability filter.
fn gateway_model(raw: &RawValue, source: Source) -> Result<Option<Model>, Invalid> {
    if raw.get() == "null" {
        return Err(Invalid);
    }
    let gateway = matches!(source, Source::Gateway);
    let tags = member(
        raw,
        if gateway {
            "tags"
        } else {
            "supported_parameters"
        },
    )
    .filter(|tags| !gateway || tags.get().starts_with('['));
    if !input::includes(tags, if gateway { "tool-use" } else { "tools" })? {
        return Ok(None);
    }
    let id = input::text(member(raw, "id"))?;
    let mut model = if gateway {
        input::descriptor(
            id,
            "anthropic-messages",
            "vercel-ai-gateway",
            "https://ai-gateway.vercel.sh",
        )
    } else {
        input::descriptor(
            id,
            "openai-completions",
            "openrouter",
            "https://openrouter.ai/api/v1",
        )
    };
    if !gateway || member(raw, "name").is_some_and(crate::providers::json_text::is_truthy) {
        model.name = input::text(member(raw, "name"))?;
    }
    model.reasoning = input::includes(tags, "reasoning")?;
    if input::includes(
        if gateway {
            tags
        } else {
            input::field(raw, "architecture", "modality")
        },
        if gateway { "vision" } else { "image" },
    )? {
        model.input.push(ModelInput::Image);
    }
    model.cost = gateway_cost(raw, source)?;
    gateway_limits(&mut model, raw, source)?;
    Ok(Some(model))
}

/// Report one rejected selected model using an escaped external identifier.
pub(super) fn skipped(errors: &mut dyn Write, prefix: &str, id: &str) -> io::Result<()> {
    writeln!(
        errors,
        "{prefix} skipping model {}: invalid metadata",
        serde_json::to_string(id)?
    )
}

/// Map the four distinct per-token fields through the owning finite-price conversion.
fn gateway_cost(raw: &RawValue, source: Source) -> Result<crate::ModelCost, Invalid> {
    let gateway = matches!(source, Source::Gateway);
    Ok(crate::ModelCost {
        input: input::price(
            input::field(raw, "pricing", if gateway { "input" } else { "prompt" }),
            gateway,
        )?,
        output: input::price(
            input::field(
                raw,
                "pricing",
                if gateway { "output" } else { "completion" },
            ),
            gateway,
        )?,
        cache_read: input::price(input::field(raw, "pricing", "input_cache_read"), gateway)?,
        cache_write: input::price(input::field(raw, "pricing", "input_cache_write"), gateway)?,
    })
}

/// Read each gateway's distinct context and output limit fields.
fn gateway_limits(model: &mut Model, raw: &RawValue, source: Source) -> Result<(), Invalid> {
    let gateway = matches!(source, Source::Gateway);
    model.context_window = input::number(
        member(
            raw,
            if gateway {
                "context_window"
            } else {
                "context_length"
            },
        ),
        4096.0,
    )?;
    model.max_tokens = input::number(
        if gateway {
            member(raw, "max_tokens")
        } else {
            input::field(raw, "top_provider", "max_completion_tokens")
        },
        4096.0,
    )?;
    Ok(())
}

/// Escape string identifiers and retain original JSON spelling for wrong-typed identifiers.
fn diagnostic_id(id: &RawValue, original: &str, projected: &str) -> io::Result<String> {
    if let Ok(id) = input::text(Some(id)) {
        return Ok(serde_json::to_string(&id)?);
    }
    // Borrowed values are slices of projected text; escape substitutions retain byte lengths.
    let offset = id.get().as_ptr().addr() - projected.as_ptr().addr();
    original
        .get(offset..offset + id.get().len())
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("identifier range does not belong to the decoded feed"))
}

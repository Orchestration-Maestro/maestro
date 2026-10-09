//! Authored provider traversal and selected model projection.
use super::{input, routes};
use crate::Model;
use crate::providers::json_text::{is_truthy, member};
use serde_json::value::RawValue;
use std::io::{self, Write};

/// Project models in authored provider order.
pub(super) fn normalize(raw: &RawValue, errors: &mut dyn Write) -> io::Result<Vec<Model>> {
    let mut models = Vec::new();
    for &(source, provider, api, url) in routes::PROVIDERS {
        let entries = input::entries(input::field(raw, source, "models"));
        let mut canonical = if source == "kimi-for-coding" {
            entries
                .iter()
                .find(|(id, _)| id.0 == b"kimi-for-coding")
                .filter(|(_, raw)| raw.get() == "null" || eligible(raw))
                .map(|(_, raw)| {
                    project(
                        input::descriptor("kimi-for-coding".into(), api, provider, url),
                        raw,
                    )
                })
        } else {
            None
        };
        let included_canonical = canonical
            .as_ref()
            .is_some_and(|result| matches!(result, Ok(Some(_))));
        for (id, raw) in entries {
            if raw.get() != "null" && !eligible(raw) {
                continue;
            }
            let id = id.text();
            if included_canonical && ["k2p5", "k2p6"].contains(&id.as_str()) {
                continue;
            }
            let projected = if source == "kimi-for-coding" && id == "kimi-for-coding" {
                canonical.take().unwrap_or(Ok(None))
            } else {
                project(input::descriptor(id.clone(), api, provider, url), raw)
            };
            match projected {
                Ok(Some(model)) => models.push(model),
                Ok(None) => {}
                Err(_) => super::feeds::skipped(errors, "Failed to load models.dev data:", &id)?,
            }
        }
    }
    Ok(models)
}
/// Apply eligibility before interpreting optional metadata.
fn project(model: Model, raw: &RawValue) -> Result<Option<Model>, input::Invalid> {
    if raw.get() == "null" {
        return Err(input::Invalid);
    }
    let Some(mut model) = routes::route(model, raw) else {
        return Ok(None);
    };
    let alias = model.provider == "kimi-coding" && ["k2p5", "k2p6"].contains(&model.id.as_str());
    if alias {
        model.id = "kimi-for-coding".into();
        model.name = "Kimi For Coding".into();
    } else if let Some(name) = member(raw, "name").filter(|raw| is_truthy(raw)) {
        model.name = input::text(Some(name))?;
    }
    input::dev_metadata(model, raw).map(Some)
}

/// Select literal tool support before decoding an entry's identifier.
fn eligible(raw: &RawValue) -> bool {
    member(raw, "tool_call").is_some_and(|raw| raw.get() == "true")
}

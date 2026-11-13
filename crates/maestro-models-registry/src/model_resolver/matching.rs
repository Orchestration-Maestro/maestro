//! Exact reference stages and model pattern selection.
use maestro_models::Model;
use std::sync::{Arc, PoisonError, RwLock};
/// Trim authored reference whitespace.
pub(super) fn trim_reference(value: &str) -> &str {
    value.trim_matches(maestro_text::is_whitespace_scalar)
}
/// Ordered reference identity checks.
enum ReferenceStage {
    /// Whole provider/ID spelling.
    Canonical,
    /// Independently trimmed operands.
    ProviderId,
    /// Unqualified model ID.
    BareId,
}
/// Find a unique canonical, split-provider or bare-ID match in that order.
#[must_use]
pub fn find_exact_model_reference_match(
    model_reference: &str,
    available_models: &[Arc<RwLock<Model>>],
) -> Option<Arc<RwLock<Model>>> {
    let reference = trim_reference(model_reference);
    if reference.is_empty() {
        return None;
    }
    let normalized = reference.to_lowercase();
    let split = reference.split_once('/').map(|(p, id)| {
        (
            trim_reference(p).to_lowercase(),
            trim_reference(id).to_lowercase(),
        )
    });
    for stage in [
        ReferenceStage::Canonical,
        ReferenceStage::ProviderId,
        ReferenceStage::BareId,
    ] {
        let mut matches = available_models.iter().filter(|handle| {
            let model = handle.read().unwrap_or_else(PoisonError::into_inner);
            match stage {
                ReferenceStage::Canonical => {
                    format!("{}/{}", model.provider, model.id).to_lowercase() == normalized
                }
                ReferenceStage::ProviderId => split.as_ref().is_some_and(|(provider, id)| {
                    !provider.is_empty()
                        && !id.is_empty()
                        && model.provider.to_lowercase() == *provider
                        && model.id.to_lowercase() == *id
                }),
                ReferenceStage::BareId => model.id.to_lowercase() == normalized,
            }
        });
        if let Some(first) = matches.next() {
            return if matches.next().is_none() {
                Some(Arc::clone(first))
            } else {
                None
            };
        }
    }
    None
}
/// Whether an ID lacks the final eight-ASCII-digit date suffix.
fn is_alias(id: &str) -> bool {
    !id.rsplit_once('-')
        .is_some_and(|(_, suffix)| suffix.len() == 8 && suffix.bytes().all(|c| c.is_ascii_digit()))
}
/// Select exact references first, otherwise the greatest alias or dated ID.
fn try_match_model(
    pattern: &str,
    models: &[Arc<RwLock<Model>>],
    compare: &dyn Fn(&str, &str) -> std::cmp::Ordering,
) -> Option<Arc<RwLock<Model>>> {
    if let Some(model) = find_exact_model_reference_match(pattern, models) {
        return Some(model);
    }
    let lower = pattern.to_lowercase();
    let mut best: Option<(&Arc<RwLock<Model>>, String)> = None;
    for handle in models {
        let id = {
            let model = handle.read().unwrap_or_else(PoisonError::into_inner);
            if !model.id.to_lowercase().contains(&lower)
                && !model.name.to_lowercase().contains(&lower)
            {
                continue;
            }
            model.id.clone()
        };
        let replace = best.as_ref().is_none_or(|(_, previous)| {
            is_alias(&id)
                .cmp(&is_alias(previous))
                .then_with(|| compare(&id, previous))
                .is_gt()
        });
        if replace {
            best = Some((handle, id));
        }
    }
    best.map(|(handle, _)| Arc::clone(handle))
}
/// Resolve whole model text before interpreting a thinking suffix.
#[must_use]
pub fn parse_model_pattern(
    pattern: &str,
    available_models: &[Arc<RwLock<Model>>],
    options: super::ParseModelPatternOptions,
    compare_model_ids: &dyn Fn(&str, &str) -> std::cmp::Ordering,
) -> super::ParsedModelResult {
    let mut remaining = pattern;
    let mut thinking_level = None;
    let mut invalid = None;
    loop {
        if let Some(model) = try_match_model(remaining, available_models, compare_model_ids) {
            let warning = invalid.map(|(text, suffix)| format!(r#"Invalid thinking level "{suffix}" in pattern "{text}". Using default instead."#));
            return super::ParsedModelResult {
                model: Some(model),
                thinking_level: if warning.is_some() {
                    None
                } else {
                    thinking_level
                },
                warning,
            };
        }
        let Some((prefix, suffix)) = remaining.rsplit_once(':') else {
            return super::ParsedModelResult::default();
        };
        match suffix.parse() {
            Ok(level) => {
                if thinking_level.is_none() {
                    thinking_level = Some(level);
                }
            }
            Err(_) if options.allow_invalid_thinking_level_fallback => {
                if invalid.is_none() {
                    invalid = Some((remaining, suffix));
                }
            }
            Err(_) => return super::ParsedModelResult::default(),
        }
        remaining = prefix;
    }
}

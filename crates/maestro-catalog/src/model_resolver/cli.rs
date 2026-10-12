//! Provider and raw model input precedence.
use super::{
    ParseModelPatternOptions, ParsedModelResult, ResolveCliModelOptions, ResolveCliModelResult,
    parse_model_pattern,
};
use maestro_models::{DiagnosticErrorInfo, Model};
use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::{Arc, PoisonError, RwLock},
};
/// Own a selection diagnostic in the existing catalog error type.
fn failure(message: String) -> ResolveCliModelResult {
    ResolveCliModelResult {
        error: Some(
            DiagnosticErrorInfo {
                message,
                name: None,
                stack: None,
                code: None,
            }
            .into(),
        ),
        ..Default::default()
    }
}
/// Resolve command-line model input from the caller's complete catalog.
#[must_use]
pub fn resolve_cli_model(
    options: ResolveCliModelOptions<'_>,
    compare_model_ids: &dyn Fn(&str, &str) -> Ordering,
) -> ResolveCliModelResult {
    let Some(input) = options.cli_model.filter(|value| !value.is_empty()) else {
        return ResolveCliModelResult::default();
    };
    let all = options
        .model_registry
        .get_all()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if all.is_empty() {
        return failure(
            "No models available. Check your installation or add models to models.json.".into(),
        );
    }
    let providers: HashMap<_, _> = all
        .iter()
        .map(|handle| {
            let model = handle.read().unwrap_or_else(PoisonError::into_inner);
            (model.provider.to_lowercase(), model.provider.clone())
        })
        .collect();
    let explicit = options.cli_provider.filter(|value| !value.is_empty());
    if let Some(value) = explicit.filter(|value| !providers.contains_key(&value.to_lowercase())) {
        return failure(format!(
            r#"Unknown provider "{value}". Use --list-models to see available providers/models."#
        ));
    }
    select_input(input, explicit, &providers, &all, compare_model_ids)
}
/// Interpret the provider prefix before raw matching and strict pattern selection.
fn select_input(
    input: &str,
    explicit: Option<&str>,
    providers: &HashMap<String, String>,
    all: &[Arc<RwLock<Model>>],
    compare: &dyn Fn(&str, &str) -> Ordering,
) -> ResolveCliModelResult {
    let mut provider = explicit.and_then(|value| providers.get(&value.to_lowercase()));
    let mut pattern = input;
    if provider.is_none()
        && let Some((prefix, rest)) = input.split_once('/')
        && let Some(canonical) = providers.get(&prefix.to_lowercase())
    {
        provider = Some(canonical);
        pattern = rest;
    }
    if provider.is_none()
        && let Some(model) = raw_match(input, all)
    {
        return selected(ParsedModelResult {
            model: Some(model),
            ..Default::default()
        });
    }
    if let Some(provider) = provider.filter(|_| explicit.is_some())
        && let Some((boundary, _)) = input.match_indices('/').nth(provider.matches('/').count())
        && input[..boundary].to_lowercase() == provider.to_lowercase()
    {
        pattern = &input[boundary + 1..];
    }
    let candidates: Vec<_> = all
        .iter()
        .filter(|handle| {
            provider.is_none_or(|p| {
                handle
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .provider
                    == *p
            })
        })
        .cloned()
        .collect();
    let parsed = strict_match(pattern, &candidates, compare);
    if parsed.model.is_some() {
        return selected(parsed);
    }
    if explicit.is_none() && provider.is_some() {
        if let Some(model) = raw_match(input, all) {
            return selected(ParsedModelResult {
                model: Some(model),
                ..Default::default()
            });
        }
        let fallback = strict_match(input, all, compare);
        if fallback.model.is_some() {
            return selected(fallback);
        }
    }
    if let Some(provider) = provider {
        return custom_model(provider, pattern, &candidates);
    }
    failure(format!(
        r#"Model "{input}" not found. Use --list-models to see available models."#
    ))
}
/// Parse model input without dropping invalid suffixes.
fn strict_match(
    input: &str,
    all: &[Arc<RwLock<Model>>],
    compare: &dyn Fn(&str, &str) -> Ordering,
) -> ParsedModelResult {
    parse_model_pattern(
        input,
        all,
        ParseModelPatternOptions {
            allow_invalid_thinking_level_fallback: false,
        },
        compare,
    )
}
/// Transfer a parsed selection without changing its optional fields.
fn selected(result: ParsedModelResult) -> ResolveCliModelResult {
    ResolveCliModelResult {
        model: result.model,
        thinking_level: result.thinking_level,
        warning: result.warning,
        error: None,
    }
}
/// First raw-ID or canonical match, without exact-stage uniqueness.
fn raw_match(input: &str, all: &[Arc<RwLock<Model>>]) -> Option<Arc<RwLock<Model>>> {
    let lower = input.to_lowercase();
    all.iter()
        .find(|handle| {
            let model = handle.read().unwrap_or_else(PoisonError::into_inner);
            model.id.to_lowercase() == lower
                || format!("{}/{}", model.provider, model.id).to_lowercase() == lower
        })
        .cloned()
}
/// Copy the configured default's metadata, or the first provider descriptor.
fn custom_model(
    provider: &str,
    pattern: &str,
    candidates: &[Arc<RwLock<Model>>],
) -> ResolveCliModelResult {
    let default = super::DEFAULT_MODEL_PER_PROVIDER
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(provider)
        .cloned();
    let base = candidates
        .iter()
        .find(|handle| {
            default.as_ref().is_some_and(|id| {
                !id.is_empty() && handle.read().unwrap_or_else(PoisonError::into_inner).id == *id
            })
        })
        .or_else(|| candidates.first());
    let Some(base) = base else {
        return failure(format!(
            r#"Model "{provider}/{pattern}" not found. Use --list-models to see available models."#
        ));
    };
    let mut model = base.read().unwrap_or_else(PoisonError::into_inner).clone();
    model.id = pattern.into();
    model.name = pattern.into();
    ResolveCliModelResult {
        model: Some(Arc::new(RwLock::new(model))),
        warning: Some(format!(
            r#"Model "{pattern}" not found for provider "{provider}". Using custom model id."#
        )),
        ..Default::default()
    }
}

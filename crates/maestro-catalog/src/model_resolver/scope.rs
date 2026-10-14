//! Ordered model patterns resolved against authenticated models.
use super::{
    CatalogError, ModelResolverOutput, ParseModelPatternOptions, ScopedModel, diagnostic,
    parse_model_pattern,
};
use crate::ModelRegistry;
use maestro_models::{Model, ModelThinkingLevel, models_are_equal};
use maestro_pattern::minimatch_with_nocase;
use std::{
    cmp::Ordering,
    sync::{Arc, PoisonError, RwLock},
};
/// Resolve patterns against the available models, warning once per unmatched pattern.
///
/// Patterns with `*`, `?` or `[` are globs; the others follow [`parse_model_pattern`].
/// Models repeat only by provider and ID, keeping the first level.
///
/// # Errors
/// Returns an availability failure before pattern processing, or a rejected glob failure.
pub fn resolve_model_scope(
    patterns: &[String],
    model_registry: &ModelRegistry,
    compare_model_ids: &dyn Fn(&str, &str) -> Ordering,
    output: &mut ModelResolverOutput<'_>,
) -> Result<Vec<ScopedModel>, CatalogError> {
    let available = model_registry.get_available()?;
    let mut scoped: Vec<ScopedModel> = Vec::new();
    for pattern in patterns {
        let (matches, thinking_level) = if pattern.contains(['*', '?', '[']) {
            glob_matches(pattern, &available)?
        } else {
            let parsed = parse_model_pattern(
                pattern,
                &available,
                ParseModelPatternOptions::default(),
                compare_model_ids,
            );
            if let Some(warning) = parsed.warning {
                output.warn(&warning);
            }
            (parsed.model.into_iter().collect(), parsed.thinking_level)
        };
        if matches.is_empty() {
            output.warn(&format!(r#"No models match pattern "{pattern}""#));
        }
        for model in matches {
            if !scoped.iter().any(|entry| same_model(&entry.model, &model)) {
                scoped.push(ScopedModel {
                    model,
                    thinking_level: thinking_level.clone(),
                });
            }
        }
    }
    Ok(scoped)
}
/// Shared descriptors matched by one pattern with their pattern-level effort.
type Matches = (Vec<Arc<RwLock<Model>>>, Option<ModelThinkingLevel>);
/// Models whose `provider/id` or bare ID matches the glob, without a valid trailing level.
fn glob_matches(pattern: &str, available: &[Arc<RwLock<Model>>]) -> Result<Matches, CatalogError> {
    let (glob, level) = match pattern.rsplit_once(':') {
        Some((glob, suffix)) => suffix
            .parse()
            .map_or((pattern, None), |level| (glob, Some(level))),
        None => (pattern, None),
    };
    let mut matches = Vec::new();
    for handle in available {
        let model = handle.read().unwrap_or_else(PoisonError::into_inner);
        let hit = minimatch_with_nocase(&format!("{}/{}", model.provider, model.id), glob, true)
            .and_then(|by_full| Ok(by_full || minimatch_with_nocase(&model.id, glob, true)?))
            .map_err(|error| diagnostic(error.to_string()))?;
        drop(model);
        if hit {
            matches.push(Arc::clone(handle));
        }
    }
    Ok((matches, level))
}
/// Whether two handles name the same provider and ID.
fn same_model(a: &Arc<RwLock<Model>>, b: &Arc<RwLock<Model>>) -> bool {
    Arc::ptr_eq(a, b) || {
        let (a, b) = (
            a.read().unwrap_or_else(PoisonError::into_inner),
            b.read().unwrap_or_else(PoisonError::into_inner),
        );
        models_are_equal(Some(&a), Some(&b))
    }
}

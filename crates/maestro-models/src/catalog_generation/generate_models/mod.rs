//! Assemble native catalog data from normalized developer feeds.
mod emit;
mod emit_compat;
mod feeds;
mod input;
mod models_dev;
mod overrides;
mod routes;
mod thinking;
pub use feeds::{fetch_ai_gateway_models, fetch_open_router_models, load_models_dev_data};

#[cfg(test)]
mod tests;

use crate::Model;
use indexmap::IndexMap;
mod fixed_models;
/// Unique descriptors retaining their provider and model encounter order.
type Catalog = IndexMap<String, IndexMap<String, Model>>;
/// Add a descriptor only when both identity operands are absent together.
fn add_missing(models: &mut Vec<Model>, addition: Model) {
    if !models
        .iter()
        .any(|model| model.provider == addition.provider && model.id == addition.id)
    {
        models.push(addition);
    }
}
/// Compose normalized feeds and authored additions before first-wins grouping.
fn assemble(feeds: [Vec<Model>; 3]) -> Catalog {
    let mut models: Vec<_> = feeds
        .into_iter()
        .flatten()
        .filter(|model| {
            !(matches!(model.provider.as_str(), "opencode" | "opencode-go")
                && model.id == "gpt-5.3-codex-spark")
        })
        .collect();
    overrides::correct_models(&mut models);
    for (index, model) in fixed_models::initial().into_iter().enumerate() {
        if index == 9 {
            add_copilot(&mut models);
        }
        add_missing(&mut models, model);
    }
    models.extend(fixed_models::deepseek());
    overrides::deepseek_compat(&mut models);
    overrides::minimax_models(&mut models);
    models.extend(fixed_models::codex());
    for (index, model) in fixed_models::remaining().into_iter().enumerate() {
        if index < 3 {
            add_missing(&mut models, model);
        } else {
            models.push(model);
        }
    }
    let azure: Vec<_> = models
        .iter()
        .filter(|model| model.provider == "openai" && model.api == "openai-responses")
        .map(|model| Model {
            api: "azure-openai-responses".into(),
            provider: "azure-openai-responses".into(),
            base_url: String::new(),
            ..model.clone()
        })
        .collect();
    models.extend(azure);
    let mut catalog: Catalog = IndexMap::new();
    for mut model in models {
        thinking::apply_thinking_level_metadata(&mut model);
        catalog
            .entry(model.provider.clone())
            .or_default()
            .entry(model.id.clone())
            .or_insert(model);
    }
    catalog
}
/// Compare keys in UTF-16 code-unit order.
fn utf16_order(left: &str, right: &str) -> std::cmp::Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}
/// Count unique winners with canonical integer providers before encounter-ordered names.
fn provider_counts(catalog: &Catalog) -> Vec<(String, usize)> {
    let mut providers: Vec<_> = catalog.iter().collect();
    providers.sort_by_key(|(name, _)| {
        crate::providers::json_text::array_index(name).map_or((1, 0), |index| (0, index))
    });
    providers
        .into_iter()
        .map(|(name, models)| (name.clone(), models.len()))
        .collect()
}

/// Derive the missing Copilot descriptor from the first eligible base.
fn add_copilot(models: &mut Vec<Model>) {
    if models
        .iter()
        .any(|model| model.provider == "github-copilot" && model.id == "gpt-5.3-codex")
    {
        return;
    }
    if let Some(base) = models
        .iter()
        .find(|model| model.provider == "github-copilot" && model.id == "gpt-5.2-codex")
    {
        let derived = Model {
            id: "gpt-5.3-codex".into(),
            name: "GPT-5.3 Codex".into(),
            ..base.clone()
        };
        add_missing(models, derived);
    }
}

/// Retrieve normalized feeds, assemble unique descriptors and write the native catalog.
///
/// The destination is the generated directory; its parent must already exist.
/// Acquisition is sequential: models.dev, `OpenRouter`, then Vercel AI Gateway.
///
/// # Errors
/// Returns filesystem or output-writer errors without reporting later success.
/// Failed publication can leave partially written output files.
pub async fn generate_models(
    fetch: &crate::Fetch,
    destination: &std::path::Path,
    output: &mut dyn std::io::Write,
    errors: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    let dev = load_models_dev_data(fetch, output, errors).await?;
    let router = fetch_open_router_models(fetch, output, errors).await?;
    let gateway = fetch_ai_gateway_models(fetch, output, errors).await?;
    let catalog = assemble([dev, router, gateway]);
    emit::write_catalog(&catalog, destination)?;
    writeln!(output, "Generated src/catalog/models_generated/")?;
    print_statistics(&catalog, output)
}
/// Report only emitted winners, retaining the provider reporting order.
fn print_statistics(catalog: &Catalog, output: &mut dyn std::io::Write) -> std::io::Result<()> {
    writeln!(output, "\nModel Statistics:")?;
    writeln!(
        output,
        "  Total tool-capable models: {}",
        catalog.values().map(IndexMap::len).sum::<usize>()
    )?;
    writeln!(
        output,
        "  Reasoning-capable models: {}",
        catalog
            .values()
            .flat_map(IndexMap::values)
            .filter(|model| model.reasoning)
            .count()
    )?;
    for (provider, count) in provider_counts(catalog) {
        writeln!(output, "  {provider}: {count} models")?;
    }
    Ok(())
}

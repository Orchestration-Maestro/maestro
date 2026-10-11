//! Complete returned resource projection for recorded comparisons.
#![cfg(test)]
use maestro_packages::ResolvedPaths;
use serde_json::{Value, json};
/// Compare all returned paths, flags and provenance fields.
pub fn output(result: ResolvedPaths, base: &str) -> Value {
    let encode = |items: Vec<maestro_packages::ResolvedResource>| {
        items.into_iter().map(|r|json!({"path":r.path.replace(base,"$BASE"),"enabled":r.enabled,"metadata":{"source":r.metadata.source.replace(base,"$BASE"),"scope":r.metadata.scope,"origin":r.metadata.origin,"baseDir":r.metadata.base_dir.map(|s|s.replace(base,"$BASE"))}})).collect::<Vec<_>>()
    };
    json!({"extensions":encode(result.extensions),"skills":encode(result.skills),"prompts":encode(result.prompts),"themes":encode(result.themes)})
}

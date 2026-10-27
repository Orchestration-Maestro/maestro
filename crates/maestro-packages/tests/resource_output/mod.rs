//! Complete returned resource projection for recorded comparisons.
#![cfg(test)]
use maestro_packages::ResolvedPaths;
use serde_json::{Value, json};
/// Compare all returned paths, flags and provenance fields.
pub fn output(result: ResolvedPaths, base: &str) -> Value {
    let recorded = |path: &str| recorded(path, base);
    let encode = |items: Vec<maestro_packages::ResolvedResource>| {
        items.into_iter().map(|r|json!({"path":recorded(&r.path),"enabled":r.enabled,"metadata":{"source":recorded(&r.metadata.source),"scope":r.metadata.scope,"origin":r.metadata.origin,"baseDir":r.metadata.base_dir.map(|s|recorded(&s))}})).collect::<Vec<_>>()
    };
    json!({"extensions":encode(result.extensions),"skills":encode(result.skills),"prompts":encode(result.prompts),"themes":encode(result.themes)})
}

/// `path` as the recordings spell it: the base as `$BASE` and, on Windows, which never
/// stores `\\` in a name, every native separator as `/`.
pub fn recorded(path: &str, base: &str) -> String {
    let path = path.replace(base, "$BASE");
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path
    }
}

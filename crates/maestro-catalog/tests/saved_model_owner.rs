//! The one owner of saved-model restoration: no other source pairs a catalog lookup with
//! the credential check that restores a saved model.
#![cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// The file that owns the lookup-then-credential restoration.
const OWNER: &str = "maestro-catalog/src/model_resolver/restoration.rs";

/// The lines of `text` where a lookup by `.find(` is followed, within the next eight lines,
/// by the credential check.
fn lookup_then_credential_check(text: &str) -> Vec<usize> {
    let lines: Vec<&str> = text.lines().collect();
    (0..lines.len())
        .filter(|&index| lines[index].contains(".find("))
        .filter(|&index| {
            lines[index..lines.len().min(index + 9)]
                .iter()
                .any(|line| line.contains("has_configured_auth"))
        })
        .map(|index| index + 1)
        .collect()
}

/// Every Rust source below `directory`.
fn sources(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn the_pair_detector_flags_a_lookup_followed_by_its_credential_check() {
    let paired = "let saved = registry.find(provider, id);\nif let Some(model) = saved\n    && registry.has_configured_auth(&model)? {";
    assert_eq!(lookup_then_credential_check(paired), [1]);
    let alone = "if !registry.has_configured_auth(&model)? {\n    return Ok(false);\n}";
    assert!(lookup_then_credential_check(alone).is_empty());
}

#[test]
fn saved_model_restoration_has_one_owner() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut found = Vec::new();
    for member in std::fs::read_dir(crates).unwrap() {
        let source = member.unwrap().path().join("src");
        if source.is_dir() {
            sources(&source, &mut found);
        }
    }
    let bypasses: Vec<String> = found
        .iter()
        .filter(|path| !path.ends_with(OWNER))
        .flat_map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            let shown = path.strip_prefix(crates).unwrap().display().to_string();
            lookup_then_credential_check(&text)
                .into_iter()
                .map(move |line| format!("{shown}:{line}"))
        })
        .collect();
    assert!(
        bypasses.is_empty(),
        "restore through restore_saved_model: {bypasses:?}"
    );
}

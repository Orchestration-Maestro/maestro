//! Structural packaging of integration tests in every member.

use std::path::Path;

use crate::source::{Member, Source, read_manifest};

/// One top-level module declaration of an integration-test root.
pub(crate) struct Module {
    /// Declared name as written.
    pub(crate) name: String,
    /// Whether the declaration names a file instead of holding its body.
    pub(crate) out_of_line: bool,
    /// Whether it is an unconditional out-of-line module at its own file name.
    ordinary: bool,
}

/// Top-level module declarations of a member's integration-test root, for the shared walk.
///
/// Every other source records none.
pub(crate) fn modules(member: &Member, source: &Source, file: &syn::File) -> Vec<Module> {
    if source.path != member.directory.join("tests").join("integration.rs") {
        return Vec::new();
    }
    file.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) => {
                let name = module.ident.to_string();
                Some(Module {
                    out_of_line: module.content.is_none(),
                    ordinary: ordinary_module(module, &name),
                    name,
                })
            }
            _ => None,
        })
        .collect()
}

/// Check every member with top-level integration-test sources, reporting each failing member.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    let diagnostics: Vec<_> = members
        .iter()
        .filter_map(|member| check_member(member).err())
        .collect();
    if diagnostics.is_empty() {
        return Ok(());
    }
    Err(diagnostics.join("\n"))
}

/// Whether the package turns off automatic integration-test discovery.
fn autotests_disabled(manifest: &toml::Table) -> bool {
    manifest
        .get("package")
        .and_then(|package| package.get("autotests"))
        .and_then(toml::Value::as_bool)
        == Some(false)
}

/// Check the explicit Cargo target and automatic discovery policy.
fn check_manifest(manifest_path: &Path, manifest: &toml::Table) -> Result<(), String> {
    let targets = manifest.get("test").and_then(toml::Value::as_array);
    let target = targets
        .filter(|targets| targets.len() == 1)
        .and_then(|targets| targets.first())
        .and_then(toml::Value::as_table);
    if !autotests_disabled(manifest)
        || target.is_none_or(|target| {
            target.get("name").and_then(toml::Value::as_str) != Some("integration")
                || target.get("path").and_then(toml::Value::as_str) != Some("tests/integration.rs")
                || ["harness", "test"].iter().any(|key| {
                    target
                        .get(*key)
                        .is_some_and(|value| value.as_bool() != Some(true))
                })
                || target.contains_key("required-features")
        })
    {
        return Err(format!(
            "{}: require autotests=false and one runnable integration target at tests/integration.rs",
            manifest_path.display()
        ));
    }
    Ok(())
}

/// With top-level integration sources, require one runnable target and one declaration per source.
fn check_member(member: &Member) -> Result<(), String> {
    let tests = member.directory.join("tests");
    let files: Vec<_> = member
        .sources
        .iter()
        .filter(|source| source.path.parent() == Some(tests.as_path()))
        .collect();
    if files.is_empty() {
        return Ok(());
    }
    let manifest_path = member.directory.join("Cargo.toml");
    check_manifest(&manifest_path, &read_manifest(&manifest_path)?)?;
    let root_path = tests.join("integration.rs");
    let root = files
        .iter()
        .find(|source| source.path == root_path)
        .and_then(|source| source.walk.as_ref())
        .ok_or_else(|| {
            format!(
                "{}: require a parseable integration root",
                root_path.display()
            )
        })?;
    for source in files.iter().filter(|source| source.path != root_path) {
        let name = source
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{}: invalid module name", source.path.display()))?;
        let modules: Vec<_> = root
            .root_modules
            .iter()
            .filter(|module| module.name == name)
            .collect();
        if modules.len() != 1 || !modules[0].ordinary {
            return Err(format!(
                "{}: require one unconditional out-of-line module in {}",
                source.path.display(),
                root_path.display()
            ));
        }
    }
    Ok(())
}

/// Admit ordinary modules and canonical file paths, never conditional inclusions.
fn ordinary_module(module: &syn::ItemMod, name: &str) -> bool {
    module.content.is_none() && module.attrs.iter().all(|attribute| {
        if attribute.path().is_ident("doc") {
            return true;
        }
        let syn::Meta::NameValue(value) = &attribute.meta else { return false };
        let syn::Expr::Lit(literal) = &value.value else { return false };
        matches!(&literal.lit, syn::Lit::Str(path) if value.path.is_ident("path") && path.value() == format!("{name}.rs"))
    })
}

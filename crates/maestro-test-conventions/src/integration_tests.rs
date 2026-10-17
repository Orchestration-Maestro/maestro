//! Structural packaging of integration tests in migrated members.

use std::path::Path;

use crate::source::{Member, read_manifest};

/// Check every member whose manifest opts into one integration-test program.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        let manifest_path = member.directory.join("Cargo.toml");
        let manifest = read_manifest(&manifest_path)?;
        if autotests_disabled(&manifest) || declares_integration_target(&manifest) {
            check_member(member, &manifest_path, &manifest)?;
        }
    }
    Ok(())
}

/// Whether the package turns off automatic integration-test discovery.
fn autotests_disabled(manifest: &toml::Table) -> bool {
    manifest
        .get("package")
        .and_then(|package| package.get("autotests"))
        .and_then(toml::Value::as_bool)
        == Some(false)
}

/// Whether any explicit test target is named `integration`.
fn declares_integration_target(manifest: &toml::Table) -> bool {
    manifest
        .get("test")
        .and_then(toml::Value::as_array)
        .is_some_and(|targets| {
            targets.iter().any(|target| {
                target.get("name").and_then(toml::Value::as_str) == Some("integration")
            })
        })
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
fn check_member(
    member: &Member,
    manifest_path: &Path,
    manifest: &toml::Table,
) -> Result<(), String> {
    let tests = member.directory.join("tests");
    let files: Vec<_> = member
        .sources
        .iter()
        .filter(|source| source.path.parent() == Some(tests.as_path()))
        .collect();
    if files.is_empty() {
        return Ok(());
    }
    check_manifest(manifest_path, manifest)?;
    let root_path = tests.join("integration.rs");
    let root = files
        .iter()
        .find(|source| source.path == root_path)
        .and_then(|source| source.syntax.as_ref())
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
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Mod(module) if module.ident == name => Some(module),
                _ => None,
            })
            .collect();
        if modules.len() != 1 || !ordinary_module(modules[0], name) {
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

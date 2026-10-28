//! Guest-only package edges and single-export component layout.

use std::{collections::BTreeSet, path::Path};
use syn::{ext::IdentExt, visit::Visit};

use cargo_metadata::{CrateType, Metadata, TargetKind};
use proc_macro2::{TokenStream, TokenTree};

use super::{Edge, metadata::members, policy::UNIFIER};
use crate::source::{Member, cfg_test, item_attributes, test_scope};

/// Internal guest libraries available to a package on every target.
const DEPENDENCIES: &[&str] = &[
    "maestro-extensions-wasm",
    "maestro-request",
    "maestro-entry",
    "maestro-tool",
    "maestro-json",
];

/// Handles every edge involving a package before native-owner exceptions apply.
pub(super) fn dependency(edge: &Edge, packages: &BTreeSet<String>) -> Option<Result<(), String>> {
    let from = packages.contains(&edge.from);
    let to = packages.contains(&edge.to);
    if to && !from {
        return Some(Err(format!(
            "non-package crate {} must not depend on package crate {}",
            edge.from, edge.to
        )));
    }
    from.then(|| {
        if to || DEPENDENCIES.contains(&edge.to.as_str()) || edge.to == UNIFIER {
            Ok(())
        } else {
            Err(format!(
                "forbidden package dependency: {} -> {}",
                edge.from, edge.to
            ))
        }
    })
}

/// Checks reusable libraries and their one separately linked component export.
pub(crate) fn check(
    metadata: &Metadata,
    sources: &[Member],
    packages: &BTreeSet<String>,
) -> Result<(), String> {
    for package in members(metadata).filter(|package| packages.contains(package.name.as_str())) {
        let library = package
            .targets
            .iter()
            .find(|target| target.is_rlib())
            .ok_or("package needs an rlib library")?;
        let component = package.targets.iter().find(|target| {
            target.name == "component" && target.kind.contains(&TargetKind::Example)
        });
        let directory = package
            .manifest_path
            .parent()
            .ok_or("manifest has no parent")?;
        if library.crate_types != [CrateType::RLib]
            || component.is_none_or(|target| {
                target.crate_types != [CrateType::CDyLib]
                    || target.src_path != directory.join("src/component.rs")
            })
        {
            return Err(format!(
                "{}: package needs an rlib and a cdylib example component at src/component.rs",
                package.name
            ));
        }
        let member = sources
            .iter()
            .find(|member| member.name == package.name.as_str())
            .ok_or("missing package sources")?;
        check_exports(member, library.src_path.as_std_path())?;
    }
    Ok(())
}

/// Rejects exports outside the component entry and requires the reusable factory.
fn check_exports(member: &Member, library: &Path) -> Result<(), String> {
    let mut count = 0;
    let mut factory = false;
    for source in &member.sources {
        if test_scope(member, source) {
            continue;
        }
        let Some(syntax) = &source.syntax else {
            continue;
        };
        if source.path == library {
            factory = syntax.items.iter().any(public_factory);
        }
        let mut names = ImportNames::default();
        names.visit_file(syntax);
        let names = names.export_names();
        let mut found = ExportCalls {
            names: &names,
            count: 0,
        };
        found.visit_file(syntax);
        let exports = found.count;
        if exports > 0 && source.path != member.directory.join("src/component.rs") {
            return Err(format!(
                "{}: export_extension belongs only in src/component.rs",
                member.name
            ));
        }
        count += exports;
    }
    if !factory || count != 1 {
        return Err(format!(
            "{}: package needs a public factory and exactly one export_extension",
            member.name
        ));
    }
    Ok(())
}

/// Counts literal macro invocations without interpreting comments or strings.
fn nested_exports(tokens: TokenStream, names: &BTreeSet<String>) -> usize {
    let tokens: Vec<_> = tokens.into_iter().collect();
    let mut count = 0;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(name)
                if names.contains(&name.unraw().to_string())
                    && matches!(tokens.get(index + 1), Some(TokenTree::Punct(mark)) if mark.as_char() == '!') =>
            {
                count += 1;
            }
            TokenTree::Group(group) => count += nested_exports(group.stream(), names),
            _ => {}
        }
    }
    count
}

/// Explicit public factory declarations include reexports under the exposed name.
fn public_factory(item: &syn::Item) -> bool {
    match item {
        syn::Item::Fn(item) => {
            item.sig.ident.unraw() == "factory" && matches!(item.vis, syn::Visibility::Public(_))
        }
        syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
            let mut names = Vec::new();
            imports(&item.tree, &mut names);
            names.iter().any(|(_, exposed)| exposed == "factory")
        }
        _ => false,
    }
}

/// Enumerates explicit imports without guessing what a glob exposes.
fn imports(tree: &syn::UseTree, names: &mut Vec<(String, String)>) {
    match tree {
        syn::UseTree::Path(path) => imports(&path.tree, names),
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                imports(tree, names);
            }
        }
        syn::UseTree::Name(name) => names.push((
            name.ident.unraw().to_string(),
            name.ident.unraw().to_string(),
        )),
        syn::UseTree::Rename(name) => names.push((
            name.ident.unraw().to_string(),
            name.rename.unraw().to_string(),
        )),
        syn::UseTree::Glob(_) => {}
    }
}

/// File-wide macro imports used by the bounded export scan.
#[derive(Default)]
struct ImportNames {
    /// Original and locally exposed names, including chained renames.
    imports: Vec<(String, String)>,
}
impl ImportNames {
    /// Resolves explicit renames until no additional export name is exposed.
    fn export_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::from(["export_extension".into()]);
        let mut previous = 0;
        while previous != names.len() {
            previous = names.len();
            for (original, exposed) in &self.imports {
                names.extend(names.contains(original).then(|| exposed.clone()));
            }
        }
        names
    }
}
impl<'ast> Visit<'ast> for ImportNames {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        imports(&item.tree, &mut self.imports);
    }
}

/// Counts authored exports while leaving parsed test scopes out of production.
struct ExportCalls<'a> {
    /// Canonical name and known import aliases.
    names: &'a BTreeSet<String>,
    /// Literal invocations admitted so far.
    count: usize,
}
impl<'ast> Visit<'ast> for ExportCalls<'_> {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if !cfg_test(item_attributes(item)) {
            syn::visit::visit_item(self, item);
        }
    }
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| self.names.contains(&segment.ident.unraw().to_string()))
        {
            self.count += 1;
        }
        self.count += nested_exports(invocation.tokens.clone(), self.names);
    }
}

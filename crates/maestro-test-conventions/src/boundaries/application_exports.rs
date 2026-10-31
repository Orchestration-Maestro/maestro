//! Named application exports instead of unrestricted domain namespaces.

use std::collections::{BTreeMap, BTreeSet};

use syn::{ext::IdentExt, visit::Visit};

use crate::source::{Member, imports::flatten};

/// Domain crates whose frontend surface is selected by the application.
const DOMAINS: &[&str] = &["maestro-daemon", "maestro-terminal", "maestro-markdown"];

/// Reject crate aliases, module exports and wildcard escapes from domain crates.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let modules = modules(members);
    for source in members
        .iter()
        .filter(|member| member.name == "maestro-app")
        .flat_map(|member| &member.sources)
    {
        let Some(syntax) = &source.syntax else {
            continue;
        };
        let mut uses = Uses::default();
        uses.visit_file(syntax);
        let aliases = aliases(&uses.imports);
        for (path, binding, public, line) in uses.imports {
            let path = resolve(path, &aliases);
            if public && domain(&path) && (binding.is_none() || modules.contains(&path)) {
                return Err(format!(
                    "{}:{line}: use named application exports, not a domain crate, module or wildcard",
                    source.path.display()
                ));
            }
        }
    }
    Ok(())
}

/// Whether a resolved path begins in one of the bounded domain crates.
fn domain(path: &[String]) -> bool {
    path.first()
        .is_some_and(|name| DOMAINS.iter().any(|owner| owner.replace('-', "_") == *name))
}

/// Import path, bound identifier, public visibility and source line.
type Import = (Vec<String>, Option<String>, bool, usize);

/// Imports found throughout a parsed source, including inline namespaces.
#[derive(Default)]
struct Uses {
    /// Flattened uses and external-crate aliases.
    imports: Vec<Import>,
}

impl<'ast> Visit<'ast> for Uses {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        for import in flatten(&item.tree) {
            let path = import
                .path
                .iter()
                .filter(|name| **name != "self")
                .map(|name| name.unraw().to_string())
                .collect();
            self.imports.push((
                path,
                import.binding.map(|name| name.unraw().to_string()),
                matches!(item.vis, syn::Visibility::Public(_)),
                item.use_token.span.start().line,
            ));
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        let binding = item.rename.as_ref().map_or(&item.ident, |(_, name)| name);
        self.imports.push((
            vec![item.ident.unraw().to_string()],
            Some(binding.unraw().to_string()),
            matches!(item.vis, syn::Visibility::Public(_)),
            item.extern_token.span.start().line,
        ));
    }
}

/// Resolve file-wide import chains without following a cycle indefinitely.
fn aliases(imports: &[Import]) -> BTreeMap<String, Vec<String>> {
    let mut aliases = BTreeMap::new();
    for _ in 0..imports.len() {
        for (path, binding, _, _) in imports {
            let resolved = resolve(path.clone(), &aliases);
            if domain(&resolved)
                && let Some(binding) = binding
            {
                aliases.insert(binding.clone(), resolved);
            }
        }
    }
    aliases
}

/// Replace the leading import alias with its known domain path.
fn resolve(path: Vec<String>, aliases: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let Some(prefix) = path.first().and_then(|name| aliases.get(name)) else {
        return path;
    };
    prefix.iter().chain(&path[1..]).cloned().collect()
}

/// Discover domain module paths from source layout and inline declarations.
fn modules(members: &[Member]) -> BTreeSet<Vec<String>> {
    let mut modules = BTreeSet::new();
    for member in members
        .iter()
        .filter(|member| DOMAINS.contains(&member.name.as_str()))
    {
        let root = member.name.replace('-', "_");
        modules.insert(vec![root.clone()]);
        for source in &member.sources {
            let Ok(relative) = source.path.strip_prefix(member.directory.join("src")) else {
                continue;
            };
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let mut path = vec![root.clone()];
            path.extend(
                relative
                    .with_extension("")
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned()),
            );
            if path
                .last()
                .is_some_and(|name| name == "lib" || name == "mod")
            {
                path.pop();
            }
            module_items(&syntax.items, &mut path, &mut modules);
        }
    }
    modules
}

/// Add inline and out-of-line module declarations below their logical parent.
fn module_items(items: &[syn::Item], path: &mut Vec<String>, modules: &mut BTreeSet<Vec<String>>) {
    for item in items {
        if let syn::Item::Mod(module) = item {
            path.push(module.ident.unraw().to_string());
            modules.insert(path.clone());
            if let Some((_, items)) = &module.content {
                module_items(items, path, modules);
            }
            path.pop();
        }
    }
}

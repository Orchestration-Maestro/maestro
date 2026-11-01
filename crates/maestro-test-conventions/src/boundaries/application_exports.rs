//! Named application exports instead of unrestricted domain namespaces.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use syn::{ext::IdentExt, visit::Visit};

use crate::source::{Member, imports::flatten};

/// Domain crates whose frontend surface is selected by the application.
const DOMAINS: &[&str] = &["maestro-daemon", "maestro-terminal", "maestro-markdown"];

/// Reject crate aliases, module exports and wildcard escapes from domain crates.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    let modules = modules(members);
    let sources: Vec<_> = members
        .iter()
        .filter(|member| member.name == "maestro-app")
        .flat_map(|member| member.sources.iter().map(move |source| (member, source)))
        .filter_map(|(member, source)| {
            let syntax = source.syntax.as_ref()?;
            let relative = source
                .path
                .strip_prefix(member.directory.join("src"))
                .or_else(|_| source.path.strip_prefix(&member.directory))
                .ok()?;
            let mut uses = Uses {
                module: module_path(relative),
                ..Uses::default()
            };
            uses.visit_file(syntax);
            Some((source, uses))
        })
        .collect();
    let imports: Vec<_> = sources.iter().flat_map(|(_, uses)| &uses.imports).collect();
    let aliases = aliases(&imports, &modules);
    for (source, uses) in sources {
        for import in uses.imports {
            let path = resolve(&import.path, &import.module, &aliases);
            if import.public
                && domain(&path)
                && (import.binding.is_none() || modules.contains(&path))
            {
                return Err(format!(
                    "{}:{}: use named application exports, not a domain crate, module or wildcard",
                    source.path.display(),
                    import.line
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

/// One import in its logical module scope.
struct Import {
    /// Import path with a trailing grouped `self` removed.
    path: Vec<String>,
    /// Bound identifier, absent for a wildcard.
    binding: Option<String>,
    /// Logical module containing the import.
    module: Vec<String>,
    /// Whether the import has unrestricted public visibility.
    public: bool,
    /// One-based source line.
    line: usize,
}

/// Imports found throughout a parsed source, including inline namespaces.
#[derive(Default)]
struct Uses {
    /// Flattened uses and external-crate aliases.
    imports: Vec<Import>,
    /// Logical module currently being visited.
    module: Vec<String>,
}

impl<'ast> Visit<'ast> for Uses {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.module.push(item.ident.unraw().to_string());
        syn::visit::visit_item_mod(self, item);
        self.module.pop();
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        for import in flatten(&item.tree) {
            let mut path: Vec<_> = import
                .path
                .iter()
                .map(|name| name.unraw().to_string())
                .collect();
            if path.last().is_some_and(|name| name == "self") {
                path.pop();
            }
            self.imports.push(Import {
                path,
                binding: import.binding.map(|name| name.unraw().to_string()),
                module: self.module.clone(),
                public: matches!(item.vis, syn::Visibility::Public(_)),
                line: item.use_token.span.start().line,
            });
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        let binding = item.rename.as_ref().map_or(&item.ident, |(_, name)| name);
        self.imports.push(Import {
            path: vec![item.ident.unraw().to_string()],
            binding: Some(binding.unraw().to_string()),
            module: self.module.clone(),
            public: matches!(item.vis, syn::Visibility::Public(_)),
            line: item.extern_token.span.start().line,
        });
    }
}

/// Index module-qualified aliases across source files, including glob-imported modules.
fn aliases(
    imports: &[&Import],
    modules: &BTreeSet<Vec<String>>,
) -> BTreeMap<Vec<String>, Vec<String>> {
    let mut aliases = BTreeMap::new();
    for import in imports {
        if let Some(binding) = &import.binding {
            let mut key = import.module.clone();
            key.push(binding.clone());
            aliases.insert(key, qualify(&import.path, &import.module));
        }
    }
    for _ in 0..imports.len() {
        let before = aliases.len();
        for import in imports.iter().filter(|import| import.binding.is_none()) {
            let path = resolve(&import.path, &import.module, &aliases);
            for module in modules.iter().filter(|module| {
                domain(&path) && module.len() == path.len() + 1 && module.starts_with(&path)
            }) {
                let mut key = import.module.clone();
                key.extend_from_slice(&module[path.len()..]);
                aliases.entry(key).or_insert_with(|| module.clone());
            }
        }
        if aliases.len() == before {
            break;
        }
    }
    aliases
}

/// Replace module-qualified alias prefixes without following a cycle indefinitely.
fn resolve(
    path: &[String],
    module: &[String],
    aliases: &BTreeMap<Vec<String>, Vec<String>>,
) -> Vec<String> {
    let mut path = qualify(path, module);
    let mut seen = BTreeSet::new();
    for _ in 0..aliases.len() {
        if domain(&path) {
            break;
        }
        let Some((length, prefix)) = (1..=path.len())
            .rev()
            .find_map(|length| aliases.get(&path[..length]).map(|prefix| (length, prefix)))
        else {
            break;
        };
        if !seen.insert(path[..length].to_vec()) {
            break;
        }
        path = prefix.iter().chain(&path[length..]).cloned().collect();
    }
    path
}

/// Anchor relative import prefixes in their logical module, retaining external domain roots.
fn qualify(path: &[String], module: &[String]) -> Vec<String> {
    if domain(path) {
        return path.to_vec();
    }
    let mut qualified = module.to_vec();
    let mut tail = path;
    while let Some((name, rest)) = tail.split_first() {
        match name.as_str() {
            "crate" => qualified.clear(),
            "self" => {}
            "super" => {
                qualified.pop();
            }
            _ => break,
        }
        tail = rest;
    }
    qualified.extend_from_slice(tail);
    qualified
}

/// Logical module path for a Rust source relative to its source root.
fn module_path(relative: &Path) -> Vec<String> {
    let mut path: Vec<_> = relative
        .with_extension("")
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if path
        .last()
        .is_some_and(|name| name == "lib" || name == "mod")
    {
        path.pop();
    }
    path
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
            path.extend(module_path(relative));
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

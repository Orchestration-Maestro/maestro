//! Named application exports instead of unrestricted domain namespaces.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use syn::{ext::IdentExt, visit::Visit};

use crate::source::{Member, imports::flatten};

/// Domain crates whose frontend surface is selected by the application.
const DOMAINS: &[&str] = &["maestro-daemon", "maestro-terminal", "maestro-markdown"];

/// Binding paths with their visibility scopes; `None` means unrestricted visibility.
type Scopes = BTreeMap<Vec<String>, Option<Vec<String>>>;

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
            if import.scope.is_none()
                && domain(&path)
                && (import.binding.is_none() || modules.contains_key(&path))
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
    /// Whether this binding comes from an external-crate declaration.
    external: bool,
    /// Narrowest module from which the binding is visible, absent for public imports.
    scope: Option<Vec<String>>,
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
                external: false,
                scope: scope(&item.vis, &self.module),
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
            external: true,
            scope: scope(&item.vis, &self.module),
            line: item.extern_token.span.start().line,
        });
    }
}

/// Scoped bindings and the root external-crate names available in every module.
#[derive(Default)]
struct Aliases {
    /// Qualified binding keys and their qualified targets.
    paths: BTreeMap<Vec<String>, Vec<String>>,
    /// Visibility of declared and glob-imported bindings.
    scopes: Scopes,
    /// Root external-crate bindings, distinct from ordinary use aliases.
    external: BTreeSet<String>,
}

/// Index module-qualified aliases across source files, including glob-imported bindings.
fn aliases(imports: &[&Import], modules: &Scopes) -> Aliases {
    let mut aliases = Aliases {
        paths: modules
            .keys()
            .filter(|module| !module.is_empty() && !domain(module))
            .map(|module| (module.clone(), module.clone()))
            .collect(),
        scopes: modules.clone(),
        ..Aliases::default()
    };
    for import in imports {
        if let Some(binding) = &import.binding {
            let mut key = import.module.clone();
            key.push(binding.clone());
            aliases.scopes.insert(key.clone(), import.scope.clone());
            aliases.paths.insert(key, import.path.clone());
            if import.external && import.module.is_empty() {
                aliases.external.insert(binding.clone());
            }
        }
    }
    for _ in 0..imports.len() {
        qualify_aliases(imports, &mut aliases);
        let before = aliases.paths.len();
        for import in imports.iter().filter(|import| import.binding.is_none()) {
            let path = resolve(&import.path, &import.module, &aliases);
            let bindings: Vec<_> = modules
                .keys()
                .chain(aliases.paths.keys())
                .filter(|key| key.len() == path.len() + 1 && key.starts_with(&path))
                .filter(|key| visible(key, &import.module, &aliases.scopes))
                .map(|key| {
                    let mut binding = import.module.clone();
                    binding.extend_from_slice(&key[path.len()..]);
                    (
                        binding,
                        key.clone(),
                        glob_scope(import, key, &aliases.scopes),
                    )
                })
                .collect();
            for (key, target, scope) in bindings {
                aliases.scopes.entry(key.clone()).or_insert(scope);
                aliases.paths.entry(key).or_insert(target);
            }
        }
        if aliases.paths.len() == before {
            break;
        }
    }
    qualify_aliases(imports, &mut aliases);
    aliases
}

/// Whether a binding's visibility scope includes the importing module.
fn visible(key: &[String], module: &[String], scopes: &Scopes) -> bool {
    scopes
        .get(key)
        .is_some_and(|scope| scope.as_ref().is_none_or(|scope| module.starts_with(scope)))
}

/// Keep the narrower ancestor scope of the glob import and its source binding.
fn glob_scope(import: &Import, key: &[String], scopes: &Scopes) -> Option<Vec<String>> {
    import
        .scope
        .iter()
        .chain(scopes.get(key).and_then(Option::as_ref))
        .max_by_key(|scope| scope.len())
        .cloned()
}

/// Qualify original alias targets against the currently known local bindings.
fn qualify_aliases(imports: &[&Import], aliases: &mut Aliases) {
    let paths: Vec<_> = imports
        .iter()
        .filter_map(|import| {
            let binding = import.binding.as_ref()?;
            let mut key = import.module.clone();
            key.push(binding.clone());
            Some((key, qualify(&import.path, &import.module, aliases)))
        })
        .collect();
    aliases.paths.extend(paths);
}

/// Replace module-qualified alias prefixes without following a cycle indefinitely.
fn resolve(path: &[String], module: &[String], aliases: &Aliases) -> Vec<String> {
    let mut path = qualify(path, module, aliases);
    let mut seen = BTreeSet::new();
    for _ in 0..aliases.paths.len() {
        if domain(&path) {
            break;
        }
        let Some((length, prefix)) = (1..=path.len()).rev().find_map(|length| {
            aliases
                .paths
                .get(&path[..length])
                .map(|prefix| (length, prefix))
        }) else {
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
fn qualify(path: &[String], module: &[String], aliases: &Aliases) -> Vec<String> {
    let mut binding = module.to_vec();
    binding.extend(path.first().cloned());
    if domain(path)
        || (path
            .first()
            .is_some_and(|name| aliases.external.contains(name))
            && !aliases.paths.contains_key(&binding))
    {
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

/// Resolve a visibility restriction to its module scope.
fn scope(visibility: &syn::Visibility, module: &[String]) -> Option<Vec<String>> {
    match visibility {
        syn::Visibility::Public(_) => None,
        syn::Visibility::Inherited => Some(module.to_vec()),
        syn::Visibility::Restricted(visibility) => {
            let path: Vec<_> = visibility
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.unraw().to_string())
                .collect();
            Some(qualify(&path, module, &Aliases::default()))
        }
    }
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

/// Discover domain and application module paths from source layout and inline declarations.
fn modules(members: &[Member]) -> Scopes {
    let mut modules = BTreeMap::new();
    for member in members
        .iter()
        .filter(|member| member.name == "maestro-app" || DOMAINS.contains(&member.name.as_str()))
    {
        let root: Vec<_> = (member.name != "maestro-app")
            .then(|| member.name.replace('-', "_"))
            .into_iter()
            .collect();
        modules.insert(root.clone(), None);
        for source in &member.sources {
            let Ok(relative) = source.path.strip_prefix(member.directory.join("src")) else {
                continue;
            };
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let mut path = root.clone();
            path.extend(module_path(relative));
            module_items(&syntax.items, &mut path, &mut modules);
        }
    }
    modules
}

/// Add inline and out-of-line module declarations below their logical parent.
fn module_items(items: &[syn::Item], path: &mut Vec<String>, modules: &mut Scopes) {
    for item in items {
        if let syn::Item::Mod(module) = item {
            let scope = scope(&module.vis, path);
            path.push(module.ident.unraw().to_string());
            modules.insert(path.clone(), scope);
            if let Some((_, items)) = &module.content {
                module_items(items, path, modules);
            }
            path.pop();
        }
    }
}

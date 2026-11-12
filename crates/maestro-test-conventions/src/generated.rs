use std::collections::BTreeMap;
use std::path::Path;

use syn::punctuated::Punctuated;

use crate::source::{Member, Source, read_manifest as read};

/// Crate that generates the guest side of the extension bindings.
pub(crate) const GUEST: &str = "maestro-extensions-wasm";

/// Crates that generate bindings from the interface files and hold their only lint exception.
const GENERATED: [&str; 2] = [GUEST, "maestro-extensions-wasmtime"];

/// Whether a member generates extension bindings.
pub(crate) fn generates(name: &str) -> bool {
    GENERATED.contains(&name)
}

/// Lints the manifest lowers from `forbid` to `deny` so the generated bindings can compile.
const LOWERED: &[&str] = &["pedantic", "too_many_arguments", "excessive_nesting"];

/// The one lint the generated bindings file may allow.
const ALLOWED: &str = "clippy::same_length_and_capacity";

/// Lint level and priority of one manifest entry.
type Level = (String, i64);

/// Require an exceptional crate to preserve every unrelated workspace lint.
pub(crate) fn check_manifest(name: &str, manifest: &Path, root: &Path) -> Result<(), String> {
    let workspace = read(&root.join("Cargo.toml"))?;
    let member = read(manifest)?;
    let own = member.get("lints").and_then(toml::Value::as_table);
    if own.is_none_or(|lints| lints.contains_key("workspace")) {
        return Err(format!("{name}: must declare its own lint table"));
    }
    let groups = workspace
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .and_then(toml::Value::as_table)
        .ok_or("missing workspace lint table")?;
    if own.is_none_or(|own| !own.keys().eq(groups.keys())) {
        return Err(format!(
            "{name}: lint groups must equal the workspace table"
        ));
    }
    for group in groups.keys().map(String::as_str) {
        let mut expected = levels(
            workspace
                .get("workspace")
                .and_then(|w| w.get("lints")?.get(group)),
        );
        let (changed, level): (&[&str], &str) = match (generates(name), group) {
            (true, "clippy") => (LOWERED, "deny"),
            _ => (&[], ""),
        };
        for lint in changed {
            expected
                .entry((*lint).to_owned())
                .and_modify(|entry| level.clone_into(&mut entry.0));
        }
        let actual = levels(own.and_then(|lints| lints.get(group)));
        if expected != actual {
            return Err(format!(
                "{name}: lints.{group} must equal the workspace table except its named binding lints"
            ));
        }
    }
    Ok(())
}

/// Normalize the entries of one lint group to levels and priorities.
fn levels(group: Option<&toml::Value>) -> BTreeMap<String, Level> {
    let Some(group) = group.and_then(toml::Value::as_table) else {
        return BTreeMap::new();
    };
    group
        .iter()
        .map(|(name, entry)| {
            let level = entry.as_str().or_else(|| entry.get("level")?.as_str());
            let priority = entry.get("priority").and_then(toml::Value::as_integer);
            (
                name.clone(),
                (
                    level.unwrap_or_default().to_owned(),
                    priority.unwrap_or_default(),
                ),
            )
        })
        .collect()
}

/// One lint attribute: its level and the lints it names.
pub(crate) struct LintAttribute {
    /// Attribute level such as `allow` or `forbid`.
    level: String,
    /// Lint paths named by the attribute.
    lints: Vec<String>,
}

/// Lint-level attributes and root layout of one source file, gathered by the shared syntax walk.
pub(crate) struct Attributes {
    /// Every lint-level attribute, in source order.
    levels: Vec<LintAttribute>,
    /// Whether the file declares modules and re-exports only, with no crate-wide lint level.
    wiring_only: bool,
    /// Whether the file's own attributes forbid exactly the lowered lints again.
    restores_forbid: bool,
}

impl Attributes {
    /// Start the collection of one parsed file with its root layout.
    pub(crate) fn new(file: &syn::File) -> Self {
        Self {
            levels: Vec::new(),
            wiring_only: wiring_only(file),
            restores_forbid: restores_forbid(file),
        }
    }

    /// Record one attribute when it sets a lint level.
    pub(crate) fn attribute(&mut self, attribute: &syn::Attribute) {
        self.levels.extend(lint_attribute(attribute));
    }
}

/// The level and lint paths of an attribute that sets a lint level.
fn lint_attribute(attribute: &syn::Attribute) -> Option<LintAttribute> {
    let level = attribute.path().get_ident()?.to_string();
    if !matches!(
        level.as_str(),
        "allow" | "warn" | "deny" | "forbid" | "expect"
    ) {
        return None;
    }
    let lints = attribute
        .parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
        .map(|metas| {
            metas
                .iter()
                .filter_map(|meta| match meta {
                    syn::Meta::Path(path) => Some(path_text(path)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    Some(LintAttribute { level, lints })
}

/// A path written with `::` separators.
fn path_text(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Check one source file of a generated-bindings crate against the exception.
pub(crate) fn check_source(member: &Member, source: &Source) -> Result<(), String> {
    let relative = source
        .path
        .strip_prefix(&member.directory)
        .map_err(|error| error.to_string())?;
    let attributes = source
        .walk
        .as_ref()
        .and_then(|walk| walk.guest.as_ref())
        .ok_or_else(|| {
            format!(
                "{}: cannot parse a generated-bindings crate source",
                source.path.display()
            )
        })?;
    let at = source.path.display();
    if bindings_source(member, source) {
        let widened = attributes
            .levels
            .iter()
            .any(|attribute| attribute.level != "allow" || attribute.lints != [ALLOWED]);
        return if widened {
            Err(format!(
                "{at}: the generated bindings file may only allow {ALLOWED}"
            ))
        } else {
            Ok(())
        };
    }
    if relative == Path::new("src/lib.rs") {
        return if attributes.wiring_only {
            Ok(())
        } else {
            Err(format!(
                "{at}: the crate root may only declare modules and re-exports"
            ))
        };
    }
    if !crate_or_module_root(relative) || attributes.restores_forbid {
        Ok(())
    } else {
        Err(format!(
            "{at}: every handwritten file must forbid pedantic, too_many_arguments and \
             excessive_nesting"
        ))
    }
}

/// Whether a file starts a lint scope: a top-level module, a test crate root or a fixture
/// component under the guest's `tests/support`. Files below a module inherit its levels.
fn crate_or_module_root(relative: &Path) -> bool {
    let parts: Vec<_> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect();
    match parts.as_slice() {
        [dir, _] => matches!(dir.as_ref(), "src" | "tests"),
        [dir, folder, file] => {
            (dir == "src" && file == "mod.rs") || (dir == "tests" && folder == "support")
        }
        _ => false,
    }
}

/// Whether a file starts by forbidding exactly the lowered lints again.
fn restores_forbid(file: &syn::File) -> bool {
    file.attrs
        .iter()
        .filter_map(lint_attribute)
        .any(|mut attribute| {
            attribute.lints.sort();
            attribute.level == "forbid"
                && attribute.lints
                    == [
                        "clippy::excessive_nesting",
                        "clippy::pedantic",
                        "clippy::too_many_arguments",
                    ]
        })
}

/// Whether the crate root declares modules and re-exports only, with no crate-wide lint level.
fn wiring_only(file: &syn::File) -> bool {
    file.items.iter().all(|item| {
        matches!(item, syn::Item::Mod(module) if module.content.is_none())
            || matches!(item, syn::Item::Use(_) | syn::Item::ExternCrate(_))
    }) && file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
}

/// Identify generated catalog source with the existing owner and path rule.
pub(crate) fn catalog_source(member: &Member, source: &Source) -> bool {
    member.name == "maestro-models"
        && source
            .path
            .starts_with(member.directory.join("src/catalog/models_generated"))
}

/// Identify the generated binding source of either extension crate exactly.
pub(crate) fn bindings_source(member: &Member, source: &Source) -> bool {
    generates(&member.name) && source.path == member.directory.join("src/bindings.rs")
}

use std::collections::BTreeMap;
use std::path::Path;

use syn::punctuated::Punctuated;
use syn::visit::Visit;

use crate::source::{Member, Source};

/// Crate that generates the extension bindings and holds their only lint exception.
pub(crate) const GUEST: &str = "maestro-extensions-wasm";

/// Lints the manifest lowers from `forbid` to `deny` so the generated bindings can compile.
const LOWERED: &[&str] = &["pedantic", "too_many_arguments", "excessive_nesting"];

/// The one lint the generated bindings file may allow.
const ALLOWED: &str = "clippy::same_length_and_capacity";

/// Lint level and priority of one manifest entry.
type Level = (String, i64);

/// Require the guest to declare the workspace lint table with only the lowered lints at `deny`.
pub(crate) fn check_manifest(manifest: &Path, root: &Path) -> Result<(), String> {
    let workspace = read(&root.join("Cargo.toml"))?;
    let guest = read(manifest)?;
    let own = guest.get("lints").and_then(toml::Value::as_table);
    if own.is_none_or(|lints| lints.contains_key("workspace")) {
        return Err(format!(
            "{GUEST}: must declare its own lint table: the workspace table with pedantic, \
             too_many_arguments and excessive_nesting at deny"
        ));
    }
    for group in ["rust", "clippy"] {
        let mut expected = levels(
            workspace
                .get("workspace")
                .and_then(|w| w.get("lints")?.get(group)),
        );
        if group == "clippy" {
            for name in LOWERED {
                expected
                    .entry((*name).to_owned())
                    .and_modify(|level| "deny".clone_into(&mut level.0));
            }
        }
        let actual = levels(own.and_then(|lints| lints.get(group)));
        if expected != actual {
            return Err(format!(
                "{GUEST}: lints.{group} must equal the workspace table with only pedantic, \
                 too_many_arguments and excessive_nesting at deny"
            ));
        }
    }
    Ok(())
}

/// Parse a manifest into a table.
fn read(path: &Path) -> Result<toml::Table, String> {
    std::fs::read_to_string(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .parse()
        .map_err(|error| format!("{}: {error}", path.display()))
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
struct LintAttribute {
    /// Attribute level such as `allow` or `forbid`.
    level: String,
    /// Lint paths named by the attribute.
    lints: Vec<String>,
}

/// Collects every lint-level attribute of a file.
#[derive(Default)]
struct Attributes(Vec<LintAttribute>);

impl<'ast> Visit<'ast> for Attributes {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        let Some(level) = attribute.path().get_ident().map(ToString::to_string) else {
            return;
        };
        if !matches!(
            level.as_str(),
            "allow" | "warn" | "deny" | "forbid" | "expect"
        ) {
            return;
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
        self.0.push(LintAttribute { level, lints });
    }
}

/// A path written with `::` separators.
fn path_text(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Check one source file of the guest crate against the exception.
pub(crate) fn check_source(member: &Member, source: &Source) -> Result<(), String> {
    let relative = source
        .path
        .strip_prefix(&member.directory)
        .map_err(|error| error.to_string())?;
    let file = source
        .syntax
        .as_ref()
        .ok_or_else(|| format!("{}: cannot parse a guest source", source.path.display()))?;
    let mut collected = Attributes::default();
    collected.visit_file(file);
    let attributes = collected.0;
    let at = source.path.display();
    if relative == Path::new("src/bindings.rs") {
        let widened = attributes
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
    if weakens_exception_lints(&attributes) {
        return Err(format!(
            "{at}: the lint exception belongs to the generated bindings file only"
        ));
    }
    if relative == Path::new("src/lib.rs") {
        return wiring_only(&source.path, file);
    }
    if !crate_or_module_root(relative) || restores_forbid(file) {
        Ok(())
    } else {
        Err(format!(
            "{at}: every handwritten guest file must forbid pedantic, too_many_arguments and \
             excessive_nesting"
        ))
    }
}

/// Whether a file starts a lint scope: a top-level module or a crate root of the guest's tests
/// and examples. Files below a module inherit its levels.
fn crate_or_module_root(relative: &Path) -> bool {
    let parts: Vec<_> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect();
    match parts.as_slice() {
        [dir, _] => matches!(dir.as_ref(), "src" | "tests" | "examples"),
        [dir, _, file] => dir == "src" && file == "mod.rs",
        _ => false,
    }
}

/// Whether any attribute below `forbid` or `deny` names a lint the exception covers.
fn weakens_exception_lints(attributes: &[LintAttribute]) -> bool {
    let covered: Vec<String> = LOWERED
        .iter()
        .map(|name| format!("clippy::{name}"))
        .chain([ALLOWED.to_owned()])
        .collect();
    attributes.iter().any(|attribute| {
        !matches!(attribute.level.as_str(), "forbid" | "deny")
            && attribute.lints.iter().any(|lint| covered.contains(lint))
    })
}

/// Whether a file starts by forbidding exactly the lowered lints again.
fn restores_forbid(file: &syn::File) -> bool {
    file.attrs.iter().any(|attribute| {
        let mut inner = Attributes::default();
        inner.visit_attribute(attribute);
        inner.0.iter().any(|attribute| {
            let mut lints = attribute.lints.clone();
            lints.sort();
            attribute.level == "forbid"
                && lints
                    == [
                        "clippy::excessive_nesting",
                        "clippy::pedantic",
                        "clippy::too_many_arguments",
                    ]
        })
    })
}

/// The root of the guest declares modules and re-exports only, with no crate-wide lint level.
fn wiring_only(path: &Path, file: &syn::File) -> Result<(), String> {
    let declares = file.items.iter().all(|item| {
        matches!(item, syn::Item::Mod(module) if module.content.is_none())
            || matches!(item, syn::Item::Use(_) | syn::Item::ExternCrate(_))
    });
    if declares
        && file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
    {
        Ok(())
    } else {
        Err(format!(
            "{}: the guest root may only declare modules and re-exports",
            path.display()
        ))
    }
}

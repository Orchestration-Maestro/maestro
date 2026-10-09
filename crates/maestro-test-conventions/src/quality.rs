use std::path::Path;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use syn::{spanned::Spanned, visit::Visit};

use crate::generated;
use crate::source::{Member, Source, cfg_test, test_layout};

/// Verify protected lint settings and production source size for every member.
pub(crate) fn check(root: &Path, members: &[Member]) -> Result<(), String> {
    check_protected_lints(root)?;
    check_json_dependencies(root, members)?;
    for member in members {
        let manifest = member.directory.join("Cargo.toml");
        if member.name == generated::GUEST {
            generated::check_manifest(&manifest, root)?;
        } else {
            check_inheritance(&member.name, &manifest)?;
        }
        for source in &member.sources {
            check_file(member, source)?;
            if member.name == generated::GUEST {
                generated::check_source(member, source)?;
            }
        }
    }
    Ok(())
}

/// Enforce the hand-written production source line limit.
fn check_file(member: &Member, source: &Source) -> Result<(), String> {
    let path = &source.path;
    if member.name == "maestro-models"
        && path.starts_with(member.directory.join("src/catalog/models_generated"))
        || test_layout(path, &member.directory)
    {
        return Ok(());
    }
    let contents = &source.contents;
    let lines = production_lines(contents, source.syntax.as_ref());
    if lines > 500 {
        return Err(format!(
            "{}: {lines} production lines exceeds 500",
            path.display()
        ));
    }
    Ok(())
}

/// Count lines with non-whitespace characters outside documentation tokens and test-only spans.
fn production_lines(contents: &str, syntax: Option<&syn::File>) -> usize {
    let mut exclusions = TestLines::default();
    if let Some(syntax) = syntax {
        exclusions.visit_file(syntax);
    }
    exclusions.spans.extend(documentation_spans(contents));
    let end = syntax
        .and_then(|syntax| syntax.items.last())
        .filter(|item| cfg_test(item_attributes(item)))
        .map_or(contents.lines().count(), |item| item.span().end().line);
    contents
        .lines()
        .take(end)
        .enumerate()
        .filter(|(line, text)| {
            // Span columns and text columns both count Unicode characters.
            text.chars().enumerate().any(|(column, character)| {
                !character.is_whitespace()
                    && !exclusions
                        .spans
                        .iter()
                        .any(|span| span.contains(&(line + 1, column)))
            })
        })
        .count()
}

/// Locates documentation tokens in the Unicode coordinates used by syntax spans.
fn documentation_spans(contents: &str) -> Vec<std::ops::Range<(usize, usize)>> {
    let mut spans = Vec::new();
    let mut offset = ra_ap_rustc_lexer::strip_shebang(contents).unwrap_or(0);
    let mut position = (1, contents[..offset].chars().count());
    for token in ra_ap_rustc_lexer::tokenize(&contents[offset..], FrontmatterAllowed::No) {
        let end = offset + token.len as usize;
        let next = contents[offset..end]
            .chars()
            .fold(position, |(line, column), character| {
                if character == '\n' {
                    (line + 1, 0)
                } else {
                    (line, column + 1)
                }
            });
        if matches!(
            token.kind,
            TokenKind::LineComment { doc_style: Some(_) }
                | TokenKind::BlockComment {
                    doc_style: Some(_),
                    ..
                }
        ) {
            spans.push(position..next);
        }
        offset = end;
        position = next;
    }
    spans
}

#[derive(Default)]
/// Source spans excluded from the production line count.
struct TestLines {
    /// Half-open character ranges occupied by test-only syntax or documentation tokens.
    spans: Vec<std::ops::Range<(usize, usize)>>,
}

impl<'ast> Visit<'ast> for TestLines {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if cfg_test(item_attributes(item)) {
            self.spans.push(span_positions(item.span()));
        } else {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        let attributes = match item {
            syn::ImplItem::Const(item) => &item.attrs,
            syn::ImplItem::Fn(item) => &item.attrs,
            syn::ImplItem::Type(item) => &item.attrs,
            syn::ImplItem::Macro(item) => &item.attrs,
            _ => return,
        };
        if cfg_test(attributes) {
            self.spans.push(span_positions(item.span()));
        } else {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        let attributes = match item {
            syn::TraitItem::Const(item) => &item.attrs,
            syn::TraitItem::Fn(item) => &item.attrs,
            syn::TraitItem::Type(item) => &item.attrs,
            syn::TraitItem::Macro(item) => &item.attrs,
            _ => return,
        };
        if cfg_test(attributes) {
            self.spans.push(span_positions(item.span()));
        } else {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) {
        let attributes = match item {
            syn::ForeignItem::Fn(item) => &item.attrs,
            syn::ForeignItem::Static(item) => &item.attrs,
            syn::ForeignItem::Type(item) => &item.attrs,
            syn::ForeignItem::Macro(item) => &item.attrs,
            _ => return,
        };
        if cfg_test(attributes) {
            self.spans.push(span_positions(item.span()));
        } else {
            syn::visit::visit_foreign_item(self, item);
        }
    }
}

/// Convert a syntax span into comparable line and character coordinates.
fn span_positions(span: proc_macro2::Span) -> std::ops::Range<(usize, usize)> {
    let start = span.start();
    let end = span.end();
    (start.line, start.column)..(end.line, end.column)
}

/// Access attributes on supported Rust item variants.
fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::TraitAlias(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

/// Require each crate to enable workspace lint inheritance.
fn check_inheritance(name: &str, manifest: &Path) -> Result<(), String> {
    let document = read_manifest(manifest)?;
    if document
        .get("lints")
        .and_then(toml::Value::as_table)
        .and_then(|lints| lints.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
    {
        return Ok(());
    }
    Err(format!("{name}: must inherit workspace lints"))
}

/// Require every protected lint to retain its forbid level.
fn check_protected_lints(root: &Path) -> Result<(), String> {
    let manifest = root.join("Cargo.toml");
    let document = read_manifest(&manifest)?;
    for (group, name) in PROTECTED {
        let lint = document
            .get("workspace")
            .and_then(|workspace| workspace.get("lints"))
            .and_then(|lints| lints.get(*group))
            .and_then(|lints| lints.get(*name));
        let level = lint.and_then(|lint| {
            lint.as_str()
                .or_else(|| lint.get("level").and_then(toml::Value::as_str))
        });
        if level != Some("forbid") {
            return Err(format!("{}: {name} must be forbid", manifest.display()));
        }
    }
    Ok(())
}

/// Lint groups and names that cannot be weakened by workspace manifests.
const PROTECTED: &[(&str, &str)] = &[
    ("rust", "unsafe_code"),
    ("rust", "forbidden_lint_groups"),
    ("clippy", "pedantic"),
    ("clippy", "missing_docs_in_private_items"),
    ("clippy", "redundant_clone"),
    ("clippy", "too_many_arguments"),
    ("clippy", "fn_params_excessive_bools"),
    ("clippy", "too_many_lines"),
    ("clippy", "cognitive_complexity"),
    ("clippy", "excessive_nesting"),
    ("clippy", "unwrap_used"),
    ("clippy", "expect_used"),
    ("clippy", "panic"),
];

/// Read a Cargo manifest through the existing native TOML parser.
fn read_manifest(manifest: &Path) -> Result<toml::Table, String> {
    let contents = std::fs::read_to_string(manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    contents
        .parse()
        .map_err(|error| format!("{}: {error}", manifest.display()))
}

/// Require canonical workspace JSON precision and normal members' inheritance of precise entries.
/// Dev/build declarations are excluded: their feature union can hide missing normal precision.
fn check_json_dependencies(root: &Path, members: &[Member]) -> Result<(), String> {
    let manifest = root.join("Cargo.toml");
    let document = read_manifest(&manifest)?;
    let dependencies = document
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"));
    let json = dependencies.and_then(|dependencies| dependencies.get("serde_json"));
    if !json.is_some_and(|entry| {
        entry
            .get("package")
            .and_then(toml::Value::as_str)
            .unwrap_or("serde_json")
            == "serde_json"
            && has_json_precision(entry)
    }) {
        return Err(format!(
            "{}: workspace.dependencies.serde_json must declare serde_json with float_roundtrip on the normal dependency path",
            manifest.display()
        ));
    }
    for member in members {
        let manifest = member.directory.join("Cargo.toml");
        let document = read_manifest(&manifest)?;
        let normal = std::iter::once(("dependencies".to_owned(), document.get("dependencies")));
        let targets = document
            .get("target")
            .and_then(toml::Value::as_table)
            .into_iter()
            .flatten()
            .map(|(selector, target)| {
                (
                    format!("target.{selector:?}.dependencies"),
                    target.get("dependencies"),
                )
            });
        for (section, entries) in normal.chain(targets) {
            let violation = entries
                .and_then(toml::Value::as_table)
                .into_iter()
                .flatten()
                .find(|(alias, entry)| json_precision_missing(alias, entry, dependencies));
            if let Some((alias, _)) = violation {
                return Err(format!(
                    "{}: {section}.{alias} must inherit workspace.dependencies.{alias} with float_roundtrip on the normal dependency path",
                    manifest.display()
                ));
            }
        }
    }
    Ok(())
}

/// Identify an explicit exact-decimal feature in one dependency entry.
fn has_json_precision(entry: &toml::Value) -> bool {
    entry
        .get("features")
        .and_then(toml::Value::as_array)
        .is_some_and(|features| {
            features
                .iter()
                .any(|feature| feature.as_str() == Some("float_roundtrip"))
        })
}

/// Select JSON by its actual package and test its own inherited workspace feature declaration.
fn json_precision_missing(
    alias: &str,
    entry: &toml::Value,
    dependencies: Option<&toml::Value>,
) -> bool {
    let inherited = entry.get("workspace").and_then(toml::Value::as_bool) == Some(true);
    let identity = if inherited {
        dependencies
            .and_then(|dependencies| dependencies.get(alias))
            .unwrap_or(entry)
    } else {
        entry
    };
    let package = identity
        .get("package")
        .and_then(toml::Value::as_str)
        .unwrap_or(alias);
    package == "serde_json" && (!inherited || !has_json_precision(identity))
}

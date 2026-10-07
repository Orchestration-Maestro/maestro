use std::path::Path;

use serde_json::Value;
use syn::{spanned::Spanned, visit::Visit};

use crate::{array, source, string};

pub(crate) fn check(root: &Path, metadata: &Value) -> Result<(), String> {
    check_protected_lints(root)?;
    let members = array(metadata, "workspace_members")?;
    for package in array(metadata, "packages")? {
        if !members.contains(&package["id"]) {
            continue;
        }
        let name = string(package, "name")?;
        let manifest = Path::new(string(package, "manifest_path")?);
        let root = manifest.parent().ok_or("manifest has no parent")?;
        check_inheritance(name, manifest)?;
        for path in source::files(root)? {
            let contents = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if path.file_name().is_some_and(|name| name == "tests.rs")
                || path
                    .strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .components()
                    .any(|part| part.as_os_str() == "tests")
            {
                continue;
            }
            let contents = contents.strip_prefix('\u{feff}').unwrap_or(&contents);
            let lines = syn::parse_file(contents).map_or_else(
                |_| contents.lines().count(),
                |syntax| production_lines(contents, &syntax),
            );
            if lines > 500 {
                return Err(format!(
                    "{}: {lines} production lines exceeds 500",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn production_lines(contents: &str, syntax: &syn::File) -> usize {
    let mut tests = TestLines::default();
    tests.visit_file(syntax);
    let end = syntax
        .items
        .last()
        .filter(|item| is_test(item_attributes(item)))
        .map_or(contents.lines().count(), |item| item.span().end().line);
    contents
        .lines()
        .take(end)
        .enumerate()
        .filter(|(line, text)| {
            // Span columns and text columns both count Unicode characters.
            text.chars().enumerate().any(|(column, character)| {
                !character.is_whitespace()
                    && !tests
                        .spans
                        .iter()
                        .any(|span| span.contains(&(line + 1, column)))
            })
        })
        .count()
}

#[derive(Default)]
struct TestLines {
    spans: Vec<std::ops::Range<(usize, usize)>>,
}

impl<'ast> Visit<'ast> for TestLines {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if is_test(item_attributes(item)) {
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
        if is_test(attributes) {
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
        if is_test(attributes) {
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
        if is_test(attributes) {
            self.spans.push(span_positions(item.span()));
        } else {
            syn::visit::visit_foreign_item(self, item);
        }
    }
}

fn span_positions(span: proc_macro2::Span) -> std::ops::Range<(usize, usize)> {
    let start = span.start();
    let end = span.end();
    (start.line, start.column)..(end.line, end.column)
}

fn is_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
}

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

fn check_inheritance(name: &str, manifest: &Path) -> Result<(), String> {
    let contents = std::fs::read_to_string(manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    let document: toml::Table = contents
        .parse()
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
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

fn check_protected_lints(root: &Path) -> Result<(), String> {
    let manifest = root.join("Cargo.toml");
    let contents = std::fs::read_to_string(&manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    let document: toml::Table = contents
        .parse()
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
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

const PROTECTED: &[(&str, &str)] = &[
    ("rust", "unsafe_code"),
    ("rust", "forbidden_lint_groups"),
    ("clippy", "pedantic"),
    ("clippy", "too_many_arguments"),
    ("clippy", "fn_params_excessive_bools"),
    ("clippy", "too_many_lines"),
    ("clippy", "cognitive_complexity"),
    ("clippy", "excessive_nesting"),
    ("clippy", "unwrap_used"),
    ("clippy", "expect_used"),
    ("clippy", "panic"),
];

use std::path::Path;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use syn::{spanned::Spanned, visit::Visit};

use crate::source::{Member, Source};

/// Verify protected lint settings and production source size for every member.
pub(crate) fn check(root: &Path, members: &[Member]) -> Result<(), String> {
    check_protected_lints(root)?;
    for member in members {
        check_inheritance(&member.name, &member.directory.join("Cargo.toml"))?;
        for source in &member.sources {
            check_file(member, source)?;
        }
    }
    Ok(())
}

/// Enforce the hand-written production source line limit.
fn check_file(member: &Member, source: &Source) -> Result<(), String> {
    let path = &source.path;
    if member.name == "maestro-models"
        && path.starts_with(member.directory.join("src/catalog/models_generated"))
        || path.file_name().is_some_and(|name| name == "tests.rs")
        || path
            .strip_prefix(&member.directory)
            .map_err(|error| error.to_string())?
            .components()
            .any(|part| part.as_os_str() == "tests")
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

/// Convert a syntax span into comparable line and character coordinates.
fn span_positions(span: proc_macro2::Span) -> std::ops::Range<(usize, usize)> {
    let start = span.start();
    let end = span.end();
    (start.line, start.column)..(end.line, end.column)
}

/// Recognize an item explicitly gated by the test configuration.
fn is_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
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

/// Require every protected lint to retain its forbid level.
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

/// Lint groups and names that cannot be weakened by workspace manifests.
const PROTECTED: &[(&str, &str)] = &[
    ("rust", "unsafe_code"),
    ("rust", "forbidden_lint_groups"),
    ("clippy", "pedantic"),
    ("clippy", "missing_docs_in_private_items"),
    ("clippy", "too_many_arguments"),
    ("clippy", "fn_params_excessive_bools"),
    ("clippy", "too_many_lines"),
    ("clippy", "cognitive_complexity"),
    ("clippy", "excessive_nesting"),
    ("clippy", "unwrap_used"),
    ("clippy", "expect_used"),
    ("clippy", "panic"),
];

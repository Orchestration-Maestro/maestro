//! Test-only environment names classified across every workspace member.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use ra_ap_rustc_lexer::{LiteralKind, TokenKind};
use syn::{Token, parse::Parser, punctuated::Punctuated};

use crate::source::{Member, Source, test_scope};

/// Test-owned item ranges and external modules gathered by the shared syntax walk.
#[derive(Default)]
pub(crate) struct Ownership {
    /// Whether inner attributes make the whole source test-owned.
    pub(crate) gated: bool,
    /// Outermost test-owned byte ranges, in traversal order.
    pub(crate) spans: Vec<Range<usize>>,
    /// Test-owned external modules, relative to the source's module directory.
    pub(crate) modules: Vec<External>,
    /// Inline module nesting around an external declaration.
    pub(crate) nesting: Vec<String>,
}

/// An external module's written path with its inherited ownership.
pub(crate) struct External {
    /// Relative authored path, including inline module directories.
    path: String,
    /// Whether a test boundary covers the declaration.
    gated: bool,
    /// Whether an explicit path is relative to the containing source file instead of its module.
    relative_to_file: bool,
}

impl External {
    /// Resolve a declaration against the source directory Rust uses for its written path.
    fn root(&self, source: &Source) -> PathBuf {
        let path = if self.relative_to_file {
            source.path.with_file_name(&self.path)
        } else {
            module_directory(source).join(&self.path)
        };
        maestro_path::normalize_native(&path)
    }
}

impl Ownership {
    /// Record an external declaration while its name is on the inline nesting stack.
    pub(crate) fn external(&mut self, item: &syn::ItemMod, gated: bool) {
        let explicit = item.attrs.iter().find_map(|attribute| {
            let syn::Meta::NameValue(pair) = &attribute.meta else {
                return None;
            };
            let syn::Expr::Lit(value) = &pair.value else {
                return None;
            };
            let syn::Lit::Str(value) = &value.lit else {
                return None;
            };
            pair.path.is_ident("path").then(|| value.value())
        });
        let relative_to_file = explicit.is_some() && self.nesting.len() == 1;
        let path = explicit.map_or_else(
            || self.nesting.join("/"),
            |path| {
                self.nesting
                    .iter()
                    .take(self.nesting.len().saturating_sub(1))
                    .map(String::as_str)
                    .chain(std::iter::once(path.as_str()))
                    .collect::<Vec<_>>()
                    .join("/")
            },
        );
        self.modules.push(External {
            path,
            gated,
            relative_to_file,
        });
    }
}

/// Whether an attribute requires a test configuration or the test-support feature.
pub(crate) fn attributes(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
            || (attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Meta>()
                    .is_ok_and(|meta| test_gate(&meta)))
    })
}

/// Whether satisfying a configuration predicate requires test-owned code.
fn test_gate(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::NameValue(pair) if pair.path.is_ident("feature") => {
            matches!(&pair.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(value) if value.value() == "test-support"))
        }
        syn::Meta::List(list) => {
            let parser = Punctuated::<syn::Meta, Token![,]>::parse_terminated;
            let Ok(predicates) = parser.parse2(list.tokens.clone()) else {
                return false;
            };
            if list.path.is_ident("all") {
                predicates.iter().any(test_gate)
            } else {
                list.path.is_ident("any")
                    && !predicates.is_empty()
                    && predicates.iter().all(test_gate)
            }
        }
        syn::Meta::NameValue(_) => false,
    }
}

/// Whether a source's authored path places it in a test-support module.
fn support_path(path: &Path) -> bool {
    path.components()
        .any(|part| part.as_os_str() == "test_support")
        || path
            .file_name()
            .is_some_and(|name| name == "test_support.rs")
}

/// Module directory used by ordinary external module declarations.
fn module_directory(source: &Source) -> PathBuf {
    let parent = source.path.parent().unwrap_or(Path::new(""));
    match source.path.file_stem().and_then(|name| name.to_str()) {
        Some("lib" | "main" | "mod") | None => parent.to_owned(),
        Some(name) => parent.join(name),
    }
}

/// Gather test-owned external module roots until nested declarations add no new roots.
fn module_roots(member: &Member) -> BTreeSet<PathBuf> {
    let mut roots = BTreeSet::new();
    loop {
        let before = roots.len();
        for source in &member.sources {
            let Some(walk) = &source.walk else { continue };
            let inherited = owned_file(member, source, &roots);
            roots.extend(
                walk.environment
                    .modules
                    .iter()
                    .filter(|module| inherited || module.gated)
                    .map(|module| module.root(source)),
            );
        }
        if roots.len() == before {
            return roots;
        }
    }
}

/// Whether a source belongs entirely to tests, including an inherited external-module gate.
fn owned_file(member: &Member, source: &Source, roots: &BTreeSet<PathBuf>) -> bool {
    test_scope(member, source)
        || support_path(&source.path)
        || source
            .walk
            .as_ref()
            .is_some_and(|walk| walk.environment.gated)
        || roots
            .iter()
            .any(|root| source.path.starts_with(root) || source.path == root.with_extension("rs"))
}

/// Report every unprefixed name occurring exclusively in test-owned Rust code.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut names: BTreeMap<String, (bool, BTreeSet<String>)> = BTreeMap::new();
    for member in members {
        let roots = module_roots(member);
        for source in &member.sources {
            collect(source, owned_file(member, source, &roots), &mut names);
        }
    }
    let diagnostics: Vec<_> = names
        .into_iter()
        .filter_map(|(name, (production, sites))| {
            (!production && !name.starts_with("MAESTRO_TEST_")).then(|| {
                format!(
                    "test-only environment variable `{name}` must start with MAESTRO_TEST_:\n{}",
                    sites.into_iter().collect::<Vec<_>>().join("\n")
                )
            })
        })
        .collect();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

/// Extract uppercase environment-name spellings, even when embedded in larger literals.
fn spellings(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices("MAESTRO_").filter_map(|(start, _)| {
        let length = text[start..]
            .bytes()
            .take_while(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'_')
            .count();
        (length > "MAESTRO_".len()).then_some(&text[start..start + length])
    })
}

/// Index identifiers, decoded strings and production documentation of dynamically built keys.
fn collect(source: &Source, owned: bool, names: &mut BTreeMap<String, (bool, BTreeSet<String>)>) {
    let mut line = 1;
    let mut offset = 0;
    for (kind, range) in &source.tokens {
        line += crate::source::count_newlines(&source.contents[offset..range.start]);
        offset = range.start;
        let text = &source.contents[range.clone()];
        let testing = owned
            || source.walk.as_ref().is_some_and(|walk| {
                walk.environment
                    .spans
                    .iter()
                    .any(|span| span.contains(&range.start))
            });
        let value = match kind {
            TokenKind::Ident | TokenKind::RawIdent => text.to_owned(),
            TokenKind::LineComment { doc_style: Some(_) }
            | TokenKind::BlockComment {
                doc_style: Some(_), ..
            } if !testing => text.to_owned(),
            TokenKind::Literal {
                kind: LiteralKind::Str { .. } | LiteralKind::RawStr { .. },
                ..
            } => {
                let Ok(literal) = syn::parse_str::<syn::LitStr>(text) else {
                    continue;
                };
                literal.value()
            }
            _ => continue,
        };
        for name in spellings(&value) {
            let (production, sites) = names.entry(name.to_owned()).or_default();
            *production |= !testing;
            sites.insert(format!("{}:{line}", source.path.display()));
        }
    }
}

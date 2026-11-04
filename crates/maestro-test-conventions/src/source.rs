use std::cell::OnceCell;
use std::ops::Range;
use std::path::{Path, PathBuf};

use cargo_metadata::Metadata;
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};

/// Flattened Rust import trees.
pub(crate) mod imports;
/// The one syntax walk that gathers every check's findings for a parsed source.
pub(crate) mod walk;

/// Loaded workspace member with its Rust sources.
pub(crate) struct Member {
    /// Cargo package name used in ownership diagnostics.
    pub(crate) name: String,
    /// Crate directory containing its manifest.
    pub(crate) directory: PathBuf,
    /// Rust sources discovered beneath this crate directory.
    pub(crate) sources: Vec<Source>,
}

/// Source text with the lexer tokens and syntax that every check reads.
pub(crate) struct Source {
    /// Source file path used in diagnostics.
    pub(crate) path: PathBuf,
    /// Normalized UTF-8 source with consistent line endings.
    pub(crate) contents: String,
    /// Lexer tokens other than whitespace, in source order, with their byte ranges.
    pub(crate) tokens: Vec<(TokenKind, Range<usize>)>,
    /// Parsed file syntax, absent for fragments or invalid source.
    pub(crate) syntax: Option<syn::File>,
    /// Findings of the shared syntax walk, gathered on first use.
    walk: OnceCell<walk::Walk>,
}

impl Source {
    /// Parse and lex normalized source text once for every check.
    pub(crate) fn new(path: PathBuf, mut contents: String) -> Self {
        let syntax = syn::parse_file(&contents).ok();
        if let Some(shebang) = syntax.as_ref().and_then(|file| file.shebang.as_ref()) {
            // Syn retains the newline but removes the shebang before assigning spans.
            contents.drain(..shebang.len());
        }
        let tokens = tokens(&contents)
            .filter(|(kind, _)| *kind != TokenKind::Whitespace)
            .collect();
        Self {
            path,
            contents,
            tokens,
            syntax,
            walk: OnceCell::new(),
        }
    }

    /// Every check's findings for this source, from one syntax walk on first use.
    ///
    /// The member is the one that holds this source; it decides which checks the walk serves.
    pub(crate) fn walk(&self, member: &Member) -> &walk::Walk {
        self.walk.get_or_init(|| walk::walk(member, self))
    }

    /// Disjoint spans of the parsed test-gated items in source order.
    pub(crate) fn test_spans(&self, member: &Member) -> &[Range<(usize, usize)>] {
        &self.walk(member).test_spans
    }
}

/// Load each workspace member's Rust files with normalized source text.
pub(crate) fn load(metadata: &Metadata) -> Result<Vec<Member>, String> {
    crate::graph::metadata::members(metadata)
        .map(|package| {
            let directory = package
                .manifest_path
                .parent()
                .ok_or("manifest has no parent")?
                .as_std_path();
            let sources = files(directory)?
                .into_iter()
                .map(|path| {
                    let contents = std::fs::read_to_string(&path)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                    let contents = contents
                        .strip_prefix('\u{feff}')
                        .unwrap_or(&contents)
                        .replace("\r\n", "\n");
                    Ok(Source::new(path, contents))
                })
                .collect::<Result<_, String>>()?;
            Ok(Member {
                name: package.name.to_string(),
                directory: directory.into(),
                sources,
            })
        })
        .collect()
}

/// Find Rust files recursively while excluding build and Git directories.
fn files(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("{}: {error}", path.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir()
            && path
                .file_name()
                .is_none_or(|name| name != "target" && name != ".git")
        {
            result.extend(files(&path)?);
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            result.push(path);
        }
    }
    result.sort();
    Ok(result)
}

/// Compute a one-based source line for a byte offset.
pub(crate) fn line(source: &str, offset: usize) -> usize {
    count_newlines(&source[..offset]) + 1
}

#[cfg(test)]
thread_local! {
    /// Bytes visited by newline scans on the current test thread.
    static SCANNED_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Lexer passes started on the current test thread.
    static LEXER_PASSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Located spans tested for a source position on the current test thread.
    pub(crate) static SPAN_TESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Syntax walks of parsed sources on the current test thread.
    static SYNTAX_WALKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Count line breaks in the supplied source slice.
pub(crate) fn count_newlines(source: &str) -> usize {
    #[cfg(test)]
    SCANNED_BYTES.set(SCANNED_BYTES.get() + source.len());
    source.bytes().filter(|byte| *byte == b'\n').count()
}

/// Yield token kinds with byte ranges in the original source, excluding its shebang.
fn tokens(source: &str) -> impl Iterator<Item = (TokenKind, Range<usize>)> {
    #[cfg(test)]
    LEXER_PASSES.set(LEXER_PASSES.get() + 1);
    let mut offset = ra_ap_rustc_lexer::strip_shebang(source).unwrap_or(0);
    ra_ap_rustc_lexer::tokenize(&source[offset..], FrontmatterAllowed::No).map(move |token| {
        let start = offset;
        offset += token.len as usize;
        (token.kind, start..offset)
    })
}

/// Line and terminated block comments with the offsets of their bodies, without inspecting literals.
pub(crate) fn comments(source: &Source) -> impl Iterator<Item = (usize, &str)> {
    source.tokens.iter().filter_map(|(kind, range)| {
        let body = match kind {
            TokenKind::LineComment { .. } => range.start + 2..range.end,
            TokenKind::BlockComment {
                terminated: true, ..
            } => range.start + 2..range.end - 2,
            _ => return None,
        };
        Some((body.start, &source.contents[body]))
    })
}

/// Whether a position lies in one of a list of disjoint spans sorted by start.
///
/// Only the last span that starts at or before the position can contain it.
pub(crate) fn covered(spans: &[Range<(usize, usize)>], position: (usize, usize)) -> bool {
    spans[..spans.partition_point(|span| span.start <= position)]
        .last()
        .is_some_and(|span| {
            #[cfg(test)]
            SPAN_TESTS.set(SPAN_TESTS.get() + 1);
            span.contains(&position)
        })
}

/// Recognize an item explicitly gated by the test configuration.
pub(crate) fn cfg_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
}

/// Recognize test source by its file name or a containing tests directory.
pub(crate) fn test_layout(path: &Path, directory: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "tests.rs")
        || path.strip_prefix(directory).is_ok_and(|relative| {
            relative
                .parent()
                .is_some_and(|parent| parent.components().any(|part| part.as_os_str() == "tests"))
        })
}

/// Recognize a source that belongs to tests: a test-only crate, a test layout or a test gate.
pub(crate) fn test_scope(member: &Member, source: &Source) -> bool {
    member.name.starts_with("maestro-test-")
        || test_layout(&source.path, &member.directory)
        || source
            .syntax
            .as_ref()
            .is_some_and(|syntax| cfg_test(&syntax.attrs))
}

/// Access attributes on supported Rust item variants.
pub(crate) fn item_attributes(item: &syn::Item) -> &[syn::Attribute] {
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

/// Emit an accessor that borrows the attributes of the supported associated item variants.
macro_rules! associated_item_attributes {
    ($name:ident, $kind:ident) => {
        /// Borrow the attributes of a supported associated item variant, or none for others.
        pub(crate) fn $name(item: &syn::$kind) -> &[syn::Attribute] {
            match item {
                syn::$kind::Const(item) => &item.attrs,
                syn::$kind::Fn(item) => &item.attrs,
                syn::$kind::Type(item) => &item.attrs,
                syn::$kind::Macro(item) => &item.attrs,
                _ => &[],
            }
        }
    };
}

associated_item_attributes!(impl_item_attributes, ImplItem);
associated_item_attributes!(trait_item_attributes, TraitItem);

/// Read a Cargo manifest through the native TOML parser.
pub(crate) fn read_manifest(manifest: &Path) -> Result<toml::Table, String> {
    std::fs::read_to_string(manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?
        .parse()
        .map_err(|error| format!("{}: {error}", manifest.display()))
}

#[cfg(test)]
mod tests;

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

#[cfg(test)]
impl Member {
    /// A member of the given identity holding each supplied source, loaded as its own in order.
    pub(crate) fn with_sources<P: Into<PathBuf>, C: Into<String>>(
        name: &str,
        directory: impl Into<PathBuf>,
        sources: impl IntoIterator<Item = (P, C)>,
    ) -> Self {
        let mut member = Self {
            name: name.into(),
            directory: directory.into(),
            sources: Vec::new(),
        };
        for (path, contents) in sources {
            let source = Source::new(&member, path.into(), contents.into());
            member.sources.push(source);
        }
        member
    }
}

/// Source text with the lexer tokens and the syntax findings that every check reads.
///
/// The parsed syntax lives only while the shared walk runs, so a source holds owned data
/// alone and loads on any thread.
pub(crate) struct Source {
    /// Source file path used in diagnostics.
    pub(crate) path: PathBuf,
    /// Normalized UTF-8 source with consistent line endings.
    pub(crate) contents: String,
    /// Lexer tokens other than whitespace, in source order, with their byte ranges.
    pub(crate) tokens: Vec<(TokenKind, Range<usize>)>,
    /// Every check's findings from the shared syntax walk, absent for fragments or invalid source.
    pub(crate) walk: Option<walk::Walk>,
}

impl Source {
    /// Parse, lex and walk normalized source text once for every check.
    ///
    /// The member is the one that holds this source; it decides which checks the walk serves.
    pub(crate) fn new(member: &Member, path: PathBuf, mut contents: String) -> Self {
        let syntax = parse(&contents);
        if let Some(shebang) = syntax.as_ref().and_then(|file| file.shebang.as_ref()) {
            // Syn retains the newline but removes the shebang before assigning spans.
            contents.drain(..shebang.len());
        }
        let tokens = tokens(&contents)
            .filter(|(kind, _)| *kind != TokenKind::Whitespace)
            .collect();
        let mut source = Self {
            path,
            contents,
            tokens,
            walk: None,
        };
        source.walk = syntax.map(|syntax| walk::walk(member, &source, &syntax));
        source
    }

    /// Disjoint spans of the parsed test-gated items in source order.
    pub(crate) fn test_spans(&self) -> &[Range<(usize, usize)>] {
        self.walk
            .as_ref()
            .map_or(&[], |walk| walk.test_spans.as_slice())
    }
}

/// Parse one source's syntax for the shared walk, counting the passes under test.
fn parse(contents: &str) -> Option<syn::File> {
    #[cfg(test)]
    SYNTAX_PARSES.set(SYNTAX_PARSES.get() + 1);
    syn::parse_file(contents).ok()
}

/// Load each workspace member's Rust files with normalized source text.
///
/// Sources load on the worker pool; members and their sources keep path order.
pub(crate) fn load(metadata: &Metadata) -> Result<Vec<Member>, String> {
    let mut members = Vec::new();
    let mut paths = Vec::new();
    for package in crate::graph::metadata::members(metadata) {
        let directory = package
            .manifest_path
            .parent()
            .ok_or("manifest has no parent")?
            .as_std_path();
        paths.extend(
            files(directory)?
                .into_iter()
                .map(|path| (members.len(), path)),
        );
        members.push(Member {
            name: package.name.to_string(),
            directory: directory.into(),
            sources: Vec::new(),
        });
    }
    let sources = crate::pool::map(&paths, |(member, path)| read(&members[*member], path))?;
    for ((member, _), source) in paths.iter().zip(sources) {
        members[*member].sources.push(source?);
    }
    Ok(members)
}

/// Read one member's Rust file as normalized source text and load it.
fn read(member: &Member, path: &Path) -> Result<Source, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let contents = contents
        .strip_prefix('\u{feff}')
        .unwrap_or(&contents)
        .replace("\r\n", "\n");
    Ok(Source::new(member, path.to_owned(), contents))
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
    /// Syntax parses of loaded sources on the current test thread.
    pub(crate) static SYNTAX_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
    test_file(member, &source.path) || source.walk.as_ref().is_some_and(|walk| walk.gated)
}

/// Recognize a file that belongs to tests by its crate or its layout alone.
fn test_file(member: &Member, path: &Path) -> bool {
    member.name.starts_with("maestro-test-") || test_layout(path, &member.directory)
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

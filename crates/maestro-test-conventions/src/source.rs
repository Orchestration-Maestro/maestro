use std::path::{Path, PathBuf};

use cargo_metadata::Metadata;
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use syn::{spanned::Spanned, visit::Visit};

/// Flattened Rust import trees.
pub(crate) mod imports;

/// Loaded workspace member with its Rust sources.
pub(crate) struct Member {
    /// Cargo package name used in ownership diagnostics.
    pub(crate) name: String,
    /// Crate directory containing its manifest.
    pub(crate) directory: PathBuf,
    /// Rust sources discovered beneath this crate directory.
    pub(crate) sources: Vec<Source>,
}

/// Source text with optional parsed Rust syntax.
pub(crate) struct Source {
    /// Source file path used in diagnostics.
    pub(crate) path: PathBuf,
    /// Normalized UTF-8 source with consistent line endings.
    pub(crate) contents: String,
    /// Parsed file syntax, absent for fragments or invalid source.
    pub(crate) syntax: Option<syn::File>,
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
                    let mut contents = contents
                        .strip_prefix('\u{feff}')
                        .unwrap_or(&contents)
                        .replace("\r\n", "\n");
                    let syntax = syn::parse_file(&contents).ok();
                    if let Some(shebang) = syntax.as_ref().and_then(|file| file.shebang.as_ref()) {
                        // Syn retains the newline but removes the shebang before assigning spans.
                        contents.drain(..shebang.len());
                    }
                    Ok(Source {
                        path,
                        contents,
                        syntax,
                    })
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
}

/// Count line breaks in the supplied source slice.
pub(crate) fn count_newlines(source: &str) -> usize {
    #[cfg(test)]
    SCANNED_BYTES.set(SCANNED_BYTES.get() + source.len());
    source.bytes().filter(|byte| *byte == b'\n').count()
}

/// Yield token kinds with byte ranges in the original source, excluding its shebang.
pub(crate) fn tokens(source: &str) -> impl Iterator<Item = (TokenKind, std::ops::Range<usize>)> {
    let mut offset = ra_ap_rustc_lexer::strip_shebang(source).unwrap_or(0);
    ra_ap_rustc_lexer::tokenize(&source[offset..], FrontmatterAllowed::No).map(move |token| {
        let start = offset;
        offset += token.len as usize;
        (token.kind, start..offset)
    })
}

/// Extract line and terminated block comments without inspecting literals.
pub(crate) fn comments(source: &str) -> Vec<(usize, &str)> {
    let mut comments = Vec::new();
    for (kind, range) in tokens(source) {
        let offset = range.start;
        let end = range.end;
        match kind {
            TokenKind::LineComment { .. } => comments.push((offset + 2, &source[offset + 2..end])),
            TokenKind::BlockComment {
                terminated: true, ..
            } => comments.push((offset + 2, &source[offset + 2..end - 2])),
            _ => {}
        }
    }
    comments
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

/// Record the item's span when it is gated by the test configuration, otherwise walk its children.
macro_rules! exclude_test_item {
    ($visitor:ident, $item:ident, $attributes:expr, $visit:ident) => {
        if $crate::source::cfg_test($attributes) {
            $visitor.spans.push(span_positions($item.span()));
        } else {
            syn::visit::$visit($visitor, $item);
        }
    };
}

/// Emit a visitor method that keeps inherited test ownership and also enables it when the item is gated by `cfg_test`.
macro_rules! inherit_test_context {
    ($visit:ident, $kind:ident, $attributes:ident) => {
        fn $visit(&mut self, item: &'ast syn::$kind) {
            let testing = self.testing;
            self.testing |= $crate::source::cfg_test($crate::source::$attributes(item));
            syn::visit::$visit(self, item);
            self.testing = testing;
        }
    };
}

/// Locate parsed test-gated items without excluding neighboring production syntax.
pub(crate) fn test_spans(syntax: Option<&syn::File>) -> Vec<std::ops::Range<(usize, usize)>> {
    let mut testing = TestSpans::default();
    if let Some(syntax) = syntax {
        testing.visit_file(syntax);
    }
    testing.spans
}

#[derive(Default)]
/// Source spans owned by test-gated items.
struct TestSpans {
    /// Half-open character ranges occupied by test-only syntax.
    spans: Vec<std::ops::Range<(usize, usize)>>,
}

impl<'ast> Visit<'ast> for TestSpans {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        exclude_test_item!(self, item, item_attributes(item), visit_item);
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        exclude_test_item!(self, item, impl_item_attributes(item), visit_impl_item);
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        exclude_test_item!(self, item, trait_item_attributes(item), visit_trait_item);
    }

    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) {
        let attributes = match item {
            syn::ForeignItem::Fn(item) => &item.attrs,
            syn::ForeignItem::Static(item) => &item.attrs,
            syn::ForeignItem::Type(item) => &item.attrs,
            syn::ForeignItem::Macro(item) => &item.attrs,
            _ => return,
        };
        exclude_test_item!(self, item, attributes, visit_foreign_item);
    }
}

/// Convert a syntax span into comparable line and character coordinates.
fn span_positions(span: proc_macro2::Span) -> std::ops::Range<(usize, usize)> {
    let start = span.start();
    let end = span.end();
    (start.line, start.column)..(end.line, end.column)
}

pub(crate) use inherit_test_context;

/// Read a Cargo manifest through the native TOML parser.
pub(crate) fn read_manifest(manifest: &Path) -> Result<toml::Table, String> {
    std::fs::read_to_string(manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?
        .parse()
        .map_err(|error| format!("{}: {error}", manifest.display()))
}

#[cfg(test)]
mod tests;

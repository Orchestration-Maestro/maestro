use std::path::{Path, PathBuf};

use cargo_metadata::Metadata;
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};

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
                    let contents = contents
                        .strip_prefix('\u{feff}')
                        .unwrap_or(&contents)
                        .replace("\r\n", "\n");
                    let syntax = syn::parse_file(&contents).ok();
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
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

/// Extract line and terminated block comments without inspecting literals.
pub(crate) fn comments(source: &str) -> Vec<(usize, &str)> {
    let mut offset = ra_ap_rustc_lexer::strip_shebang(source).unwrap_or(0);
    let mut comments = Vec::new();
    for token in ra_ap_rustc_lexer::tokenize(&source[offset..], FrontmatterAllowed::No) {
        let length = token.len as usize;
        let end = offset + length;
        match token.kind {
            TokenKind::LineComment { .. } => comments.push((offset + 2, &source[offset + 2..end])),
            TokenKind::BlockComment {
                terminated: true, ..
            } => comments.push((offset + 2, &source[offset + 2..end - 2])),
            _ => {}
        }
        offset = end;
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

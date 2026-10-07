use std::path::{Path, PathBuf};

use cargo_metadata::Metadata;
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};

pub(crate) struct Member {
    pub(crate) name: String,
    pub(crate) directory: PathBuf,
    pub(crate) sources: Vec<Source>,
}

pub(crate) struct Source {
    pub(crate) path: PathBuf,
    pub(crate) contents: String,
    pub(crate) syntax: Option<syn::File>,
}

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

pub(crate) fn line(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

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

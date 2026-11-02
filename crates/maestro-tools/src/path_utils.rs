//! Native read-path transformations for authored tool paths.

use icu_normalizer::DecomposingNormalizer;
use maestro_path::is_absolute;
use maestro_resources::paths::{home_directory, resolve_groups_with};
use std::{
    io::{self, ErrorKind},
    path::{Component, Path, PathBuf},
};

/// The no-break space that precedes AM and PM in screenshot names.
const NARROW_NO_BREAK_SPACE: char = '\u{202F}';

/// Convert a host path to text, refusing what text cannot hold.
fn utf8(path: PathBuf, what: &str) -> io::Result<String> {
    path.into_os_string()
        .into_string()
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, format!("{what} is not valid UTF-8")))
}

/// Replace U+00A0, U+2000 to U+200A, U+202F, U+205F and U+3000 with U+0020.
///
/// Other whitespace, such as tab, U+1680, U+2028 or U+FEFF, is kept.
#[must_use]
pub fn normalize_unicode_spaces(text: &str) -> String {
    text.replace(
        |c: char| {
            matches!(
                c,
                '\u{A0}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
            )
        },
        " ",
    )
}

/// Strip one leading `@` and expand a bare `~` or `~/`, keeping every other spelling.
///
/// # Errors
/// Returns an error when the home directory is needed but unknown or not UTF-8.
fn expand_written_path(file_path: &str) -> io::Result<String> {
    expand_written_path_with(file_path, home_directory)
}

/// Strip one leading `@`, then expand a leading `~` by [`maestro_path::expand_tilde`].
///
/// # Errors
/// Returns the error `home` returns for an expanded form.
fn expand_written_path_with(
    file_path: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    maestro_path::expand_tilde(file_path.strip_prefix('@').unwrap_or(file_path), home)
}

/// Strip one leading `@`, replace Unicode spaces and expand a bare `~` or `~/`.
///
/// The home directory is read only for those two forms: on Unix `HOME` as set, even
/// empty, otherwise [`std::env::home_dir`].
///
/// # Errors
/// Returns an error when the home directory is needed but unknown or not UTF-8.
pub fn expand_path(file_path: &str) -> io::Result<String> {
    expand_written_path(&normalize_unicode_spaces(file_path))
}

/// Resolve an authored path against `cwd` keeping the spelling it was written with.
fn resolve_written_path(file_path: &str, cwd: &str) -> io::Result<String> {
    let expanded = expand_written_path(file_path)?;
    if is_absolute(&expanded) {
        return Ok(expanded);
    }
    resolve_path(&[cwd, &expanded])
}

/// Resolve an authored path lexically against `cwd`, leaving absolute spellings unchanged.
///
/// The spaces [`normalize_unicode_spaces`] names are replaced by plain spaces first, so
/// the resolved spelling may differ from the authored one.
///
/// On Windows the per-drive directories (`=X:` variables) are an operand of
/// resolution, and the process working directory is read only when `cwd` and
/// those directories cannot anchor a relative result; elsewhere it is read only
/// when `cwd` is itself relative.
///
/// # Errors
/// Returns an error when expansion fails or a needed host directory is unavailable.
pub fn resolve_to_cwd(file_path: &str, cwd: &str) -> io::Result<String> {
    resolve_written_path(&normalize_unicode_spaces(file_path), cwd)
}

/// Resolve `parts` lexically right to left, reading the process working directory only
/// when the host-independent operands cannot anchor the result.
///
/// # Errors
/// Returns an error when a needed host directory is unavailable or not UTF-8.
pub(crate) fn resolve_path(parts: &[&str]) -> io::Result<String> {
    let current_directory = || utf8(std::env::current_dir()?, "working directory");
    resolve_groups_with(&[parts], current_directory)
        .map(|resolved| resolved.into_iter().collect::<String>())
}

/// Insert the narrow no-break space before a case-insensitive ` AM.` or ` PM.`.
fn narrow_space_before_meridiem(path: &str) -> String {
    let mut parts = path.split(' ');
    let mut out = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let meridiem = matches!(
            part.as_bytes(),
            [a, m, b'.', ..] if matches!(a.to_ascii_lowercase(), b'a' | b'p') && m.eq_ignore_ascii_case(&b'm')
        );
        out.push(if meridiem { NARROW_NO_BREAK_SPACE } else { ' ' });
        out.push_str(part);
    }
    out
}

/// Resolve a path under the spelling it was written with.
///
/// The written spelling is returned when the file system holds that path, and the
/// space-replaced spelling when it holds that one instead. Otherwise, when a directory
/// carries the written spelling at least as deep as the deepest existing directory of
/// the space-replaced path, that directory keeps its spelling and the rest of the path
/// takes the space-replaced one. Every other case is the space-replaced path
/// unchanged.
///
/// The directory that supplies that spelling is the deepest existing ancestor of the
/// written path, or else the directory the path was resolved against, which the caller
/// wrote: its spelling survives when the file system no longer holds it, as after the
/// working directory is removed. Once the whole authored path is reduced lexically, each
/// parent segment it still climbs cancels one component of that directory, and a
/// component the path re-authors after a climb takes the space-replaced spelling,
/// because only the directory itself was supplied. A climb that would pass the root
/// stops there. A path that expansion makes absolute was resolved against no
/// directory, so only a directory the file system holds supplies a spelling for it.
///
/// `expand` is the caller's own expansion policy, the one `resolve` applies, and decides
/// whether the result needs a directory to be resolved against. `resolve` turns one
/// authored spelling into a resolved path and must keep the spelling it is given; the
/// home and working directories are read exactly as it reads them, and its spelling of
/// the empty path is read only for an authored path that stays relative after that
/// expansion.
///
/// # Errors
/// Returns an expansion error, or the error of `resolve` for the spelling it was finally
/// called with, including the empty path when that read is needed.
pub fn resolve_existing_spelling<E, F>(file_path: &str, expand: E, resolve: F) -> io::Result<String>
where
    E: Fn(&str) -> io::Result<String>,
    F: Fn(&str) -> io::Result<String>,
{
    let as_written = resolve(file_path)?;
    let normalized_spelling = normalize_unicode_spaces(file_path);
    if normalized_spelling == file_path || Path::new(&as_written).exists() {
        return Ok(as_written);
    }
    let normalized = resolve(&normalized_spelling)?;
    if Path::new(&normalized).exists() {
        return Ok(normalized);
    }
    let expanded = expand(file_path)?;
    let base = (!is_absolute(&expanded)).then(|| resolve("")).transpose()?;
    let directory = written_directory(&as_written, base.as_deref(), &expanded)
        .filter(|directory| at_least_as_deep(directory, &normalized));
    Ok(match directory {
        Some(directory) => {
            directory.to_owned() + &normalize_unicode_spaces(&as_written[directory.len()..])
        }
        None => normalized,
    })
}

/// The directory whose written spelling is kept: the deepest existing ancestor of
/// `path`, or else the part of the directory `path` was resolved against that
/// `authored` still stands on after full lexical reduction cancels authored components.
/// Unmatched parent segments consume supplied components without climbing above root.
///
/// `base` is that directory and `authored` the spelling it was resolved from. The caller
/// wrote `base`'s spelling, so it survives even when the file system no longer holds the
/// directory, as after the working directory is removed.
fn written_directory<'a>(path: &'a str, base: Option<&'a str>, authored: &str) -> Option<&'a str> {
    let held = deepest_existing_directory(path);
    let Some(base) = base else {
        return held;
    };
    let reduced = maestro_path::normalize(authored);
    let mut supplied = Path::new(base);
    for component in Path::new(&reduced).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => supplied = supplied.parent().unwrap_or(supplied),
            Component::Normal(_) | Component::RootDir | Component::Prefix(_) => break,
        }
    }
    let Some(supplied) = supplied
        .to_str()
        .filter(|base| Path::new(path).starts_with(base))
    else {
        return held;
    };
    held.filter(|directory| Path::new(directory).starts_with(supplied))
        .or(Some(supplied))
}

/// The deepest ancestor of `path` that is an existing directory.
fn deepest_existing_directory(path: &str) -> Option<&str> {
    Path::new(path)
        .ancestors()
        .skip(1)
        .find(|ancestor| ancestor.is_dir())
        .and_then(Path::to_str)
}

/// Whether `directory` reaches at least as deep as the deepest existing directory of
/// `normalized`.
///
/// A written directory shallower than the normalized tree would create a new directory
/// beside one the file system already holds.
fn at_least_as_deep(directory: &str, normalized: &str) -> bool {
    let reached = deepest_existing_directory(normalized)
        .map_or(0, |deepest| Path::new(deepest).components().count());
    Path::new(directory).components().count() >= reached
}

/// Resolve an authored path against `cwd` under the spelling it was written with.
///
/// The order is [`resolve_existing_spelling`]'s.
///
/// # Errors
/// Returns the home-directory or working-directory error of the spelling the path was
/// finally resolved with.
pub fn resolve_existing_path(file_path: &str, cwd: &str) -> io::Result<String> {
    resolve_existing_spelling(file_path, expand_written_path, |path| {
        resolve_written_path(path, cwd)
    })
}

/// The first existing spelling among `resolved` and its macOS variants.
///
/// Variants are tried in order: the AM/PM form, NFD, curly apostrophes, and NFD with
/// curly apostrophes. A variant that repeats `resolved` costs no second check.
fn existing_variant(resolved: &str) -> Option<String> {
    if Path::new(resolved).exists() {
        return Some(resolved.to_owned());
    }
    let nfd = String::from(DecomposingNormalizer::new_nfd().normalize(resolved));
    [
        narrow_space_before_meridiem(resolved),
        nfd.clone(),
        resolved.replace('\'', "\u{2019}"),
        nfd.replace('\'', "\u{2019}"),
    ]
    .into_iter()
    .filter(|candidate| candidate.as_str() != resolved)
    .find(|candidate| Path::new(candidate).exists())
}

/// Resolve a path under the first of these candidates that the file system holds, in
/// order:
///
/// 1. the spelling [`resolve_existing_spelling`] answers with;
/// 2. that answer's macOS variants: the narrow-space AM/PM form, NFD, curly
///    apostrophes, and NFD with curly apostrophes;
/// 3. the spelling the path was written with, and those same four variants of it;
/// 4. the space-replaced path [`resolve_to_cwd`] answers with, and those same four
///    variants of it.
///
/// The list holds every candidate the reference tries, so a file the reference reads
/// still opens, and it checks the rule's answer, so a file created under a spelled
/// directory reads back. Nothing on disk returns the first candidate.
///
/// # Errors
/// Returns the home-directory, working-directory or invalid-UTF-8 error of the
/// spelling the path was resolved with.
pub fn resolve_read_path(file_path: &str, cwd: &str) -> io::Result<String> {
    let spelling = resolve_existing_path(file_path, cwd)?;
    let written = resolve_written_path(file_path, cwd)?;
    let reference = resolve_to_cwd(file_path, cwd)?;
    for candidate in [&spelling, &written, &reference] {
        if let Some(found) = existing_variant(candidate) {
            return Ok(found);
        }
    }
    Ok(spelling)
}

#[cfg(test)]
mod tests;

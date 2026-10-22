//! Native read-path transformations for authored tool paths.

use icu_normalizer::DecomposingNormalizer;
use maestro_path::{env_drive_directories, homedir, is_absolute, resolve_with};
use std::{
    io::{self, ErrorKind},
    path::{Path, PathBuf},
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

/// Join the `~` or `~/` prefix of `path` to the home directory.
///
/// Every other spelling is returned unchanged. The home directory is read only for
/// those two forms: on Unix `HOME` as set, even empty, otherwise
/// [`std::env::home_dir`].
///
/// # Errors
/// Returns an error when the home directory is needed but unknown or not UTF-8.
fn with_home(path: &str) -> io::Result<String> {
    let rest = match path.strip_prefix('~') {
        Some("") => "",
        Some(rest) if rest.starts_with('/') => rest,
        _ => return Ok(path.to_owned()),
    };
    let home = homedir(
        std::env::var_os("HOME")
            .filter(|_| cfg!(unix))
            .map(PathBuf::from),
        std::env::home_dir,
    )
    .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "home directory is unknown"))?;
    Ok(utf8(home, "home directory")? + rest)
}

/// Strip one leading `@`, replace Unicode spaces and expand a bare `~` or `~/`.
///
/// The home directory is read only for those two forms: on Unix `HOME` as set, even
/// empty, otherwise [`std::env::home_dir`].
///
/// # Errors
/// Returns an error when the home directory is needed but unknown or not UTF-8.
pub fn expand_path(file_path: &str) -> io::Result<String> {
    with_home(&normalize_unicode_spaces(
        file_path.strip_prefix('@').unwrap_or(file_path),
    ))
}

/// Strip one leading `@` and expand a bare `~` or `~/`, keeping every other spelling.
fn expand_written_path(file_path: &str) -> io::Result<String> {
    with_home(file_path.strip_prefix('@').unwrap_or(file_path))
}

/// Keep an absolute `expanded` spelling and resolve a relative one against `cwd`.
fn anchored(expanded: String, cwd: &str) -> io::Result<String> {
    if is_absolute(&expanded) {
        return Ok(expanded);
    }
    resolve_path(&[cwd, &expanded])
}

/// Resolve an authored path lexically against `cwd`, leaving absolute spellings unchanged.
///
/// On Windows the per-drive directories (`=X:` variables) are an operand of
/// resolution, and the process working directory is read only when `cwd` and
/// those directories cannot anchor a relative result; elsewhere it is read only
/// when `cwd` is itself relative.
///
/// # Errors
/// Returns an error when expansion fails or a needed host directory is unavailable.
pub fn resolve_to_cwd(file_path: &str, cwd: &str) -> io::Result<String> {
    anchored(expand_path(file_path)?, cwd)
}

/// Resolve an authored path against `cwd` keeping the spelling it was written with.
fn resolve_written_path(file_path: &str, cwd: &str) -> io::Result<String> {
    anchored(expand_written_path(file_path)?, cwd)
}

/// Resolve `parts` lexically right to left, reading the process working directory only
/// when the host-independent operands cannot anchor the result.
///
/// # Errors
/// Returns an error when a needed host directory is unavailable or not UTF-8.
pub(crate) fn resolve_path(parts: &[&str]) -> io::Result<String> {
    let directories = cfg!(windows)
        .then(|| env_drive_directories(|name| std::env::var_os(name)))
        .unwrap_or_default();
    resolve_with(
        &[parts],
        || &directories,
        || utf8(std::env::current_dir()?, "working directory"),
    )
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

/// Resolve a path under the spelling it was written with, falling back to its
/// Unicode-space normalization only when no path has the written spelling.
///
/// `resolve` turns one authored spelling into a resolved path and must keep the
/// spelling it is given; the home and working directories are read exactly as it
/// reads them.
///
/// # Errors
/// Returns the error of `resolve` for the spelling it was finally called with.
pub fn resolve_existing_spelling<F>(file_path: &str, resolve: F) -> io::Result<String>
where
    F: Fn(&str) -> io::Result<String>,
{
    let as_written = resolve(file_path)?;
    if Path::new(&as_written).exists() {
        return Ok(as_written);
    }
    resolve(&normalize_unicode_spaces(file_path))
}

/// Resolve an authored path against `cwd` under its written spelling when a path
/// has it, and otherwise under its Unicode-space normalization.
///
/// # Errors
/// Returns the home-directory, working-directory or invalid-UTF-8 error of the
/// spelling the path was finally resolved with.
pub fn resolve_existing_path(file_path: &str, cwd: &str) -> io::Result<String> {
    resolve_existing_spelling(file_path, |path| resolve_written_path(path, cwd))
}

/// Resolve like [`resolve_existing_path`], then prefer the first existing macOS-style spelling.
///
/// Candidates are tried in order: the resolved path, the AM/PM variant, the NFD
/// variant, the curly-apostrophe variant, and NFD with curly apostrophes. When none
/// exists the resolved path is returned.
///
/// # Errors
/// Returns the error of [`resolve_existing_path`].
pub fn resolve_read_path(file_path: &str, cwd: &str) -> io::Result<String> {
    let resolved = resolve_existing_path(file_path, cwd)?;
    let nfd = String::from(DecomposingNormalizer::new_nfd().normalize(&resolved));
    let candidates = [
        narrow_space_before_meridiem(&resolved),
        nfd.clone(),
        resolved.replace('\'', "\u{2019}"),
        nfd.replace('\'', "\u{2019}"),
    ];
    if Path::new(&resolved).exists() {
        return Ok(resolved);
    }
    let found = candidates
        .into_iter()
        .find(|candidate| *candidate != resolved && Path::new(candidate).exists());
    Ok(found.unwrap_or(resolved))
}

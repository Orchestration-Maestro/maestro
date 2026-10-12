//! Native read-path transformations for authored tool paths.

use icu_normalizer::DecomposingNormalizer;
use maestro_path::{Cwd, env_drive_directories, is_absolute, resolve, try_resolve};
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

/// Strip one leading `@`, replace Unicode spaces and expand a bare `~` or `~/`.
///
/// The home directory is read only for those two forms.
///
/// # Errors
/// Returns an error when the home directory is needed but unknown or not UTF-8.
pub fn expand_path(file_path: &str) -> io::Result<String> {
    let stripped = file_path.strip_prefix('@').unwrap_or(file_path);
    let normalized: String = stripped
        .chars()
        .map(|c| {
            if matches!(
                c,
                '\u{A0}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
            ) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let rest = match normalized.strip_prefix('~') {
        Some("") => "",
        Some(rest) if rest.starts_with('/') => rest,
        _ => return Ok(normalized),
    };
    let home = std::env::home_dir()
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "home directory is unknown"))?;
    Ok(utf8(home, "home directory")? + rest)
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
    let expanded = expand_path(file_path)?;
    if is_absolute(&expanded) {
        return Ok(expanded);
    }
    let directories = if cfg!(windows) {
        env_drive_directories(|name| std::env::var_os(name))
    } else {
        Vec::new()
    };
    let drives: Vec<_> = directories
        .iter()
        .map(|(letter, path)| (*letter, path.as_str()))
        .collect();
    if let Ok(resolved) = try_resolve(&[cwd, &expanded], &drives) {
        return Ok(resolved);
    }
    let process_cwd = utf8(std::env::current_dir()?, "working directory")?;
    let anchor = Cwd {
        current: &process_cwd,
        drive_directories: &drives,
    };
    Ok(resolve(&[cwd, &expanded], &anchor))
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

/// Resolve like [`resolve_to_cwd`], then prefer the first existing macOS-style spelling.
///
/// Candidates are tried in order: the resolved path, the AM/PM variant, the NFD
/// variant, the curly-apostrophe variant, and NFD with curly apostrophes. When none
/// exists the resolved path is returned.
///
/// # Errors
/// Returns the error of [`resolve_to_cwd`].
pub fn resolve_read_path(file_path: &str, cwd: &str) -> io::Result<String> {
    let resolved = resolve_to_cwd(file_path, cwd)?;
    let nfd = DecomposingNormalizer::new_nfd()
        .normalize(&resolved)
        .into_owned();
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

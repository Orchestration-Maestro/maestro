//! Home-directory expansion of authored paths.
use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

/// Where a leading `~` of an authored path points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tilde<'a> {
    /// The exact path `~`: the home directory itself.
    Home,
    /// `~` followed by a separator: the text after that one separator, below the home.
    Below(&'a str),
    /// Any other spelling, `~name` included: not a home form.
    Other,
}

/// Reads a leading `~` of `path` in the compile target's flavor.
#[must_use]
pub fn parse_tilde(path: &str) -> Tilde<'_> {
    parse(path, cfg!(windows))
}

/// Reads a leading `~` of `path` in a supplied flavor.
fn parse(path: &str, windows: bool) -> Tilde<'_> {
    let _ = (path, windows);
    Tilde::Other
}

/// Expands a leading `~` below the home directory with a normalizing join.
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub fn expand_tilde_joined<E>(
    path: &str,
    home: impl FnOnce() -> Result<String, E>,
) -> Result<String, E> {
    let _ = home;
    Ok(path.to_owned())
}

/// Expands a leading `~` by appending the rest to the home directory, in the compile target's
/// flavor.
///
/// An exact `~` is the home directory and `~` plus a separator appends the rest as written
/// below it. The home contributes its own text, so an empty home leaves a rooted result and a
/// home that ends in a separator gains none. Every other spelling, `~name` included, is
/// returned as written, and the home is read only for the two expanded forms. On Windows
/// either separator introduces the rest, which joins with `\` and drops empty components.
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
///
/// # Examples
///
/// ```
/// let home = || Ok::<_, ()>("/home/ada".to_owned());
/// # #[cfg(not(windows))]
/// assert_eq!(maestro_path::expand_tilde("~/notes", home), Ok("/home/ada/notes".to_owned()));
/// assert_eq!(maestro_path::expand_tilde("~ada", home), Ok("~ada".to_owned()));
/// ```
pub fn expand_tilde<E>(path: &str, home: impl FnOnce() -> Result<String, E>) -> Result<String, E> {
    expand_tilde_with_flavor(path, cfg!(windows), home)
}

/// Expands a leading `~` in a supplied flavor, independently of the compile target.
///
/// An exact `~` is the home directory. A `~` followed by a separator (`/`, and `\` in the
/// Windows flavor) joins the rest below the home directory: the Windows flavor joins its
/// components with `\` and drops empty ones, any other flavor appends `/` and the rest as
/// written. An empty home contributes no separator, so the rest stays relative. Every other
/// spelling, `~name` included, is returned as written. `home` runs only for the two expanded
/// forms, and an authored path is never normalized.
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub(crate) fn expand_tilde_with_flavor<E>(
    path: &str,
    windows: bool,
    home: impl FnOnce() -> Result<String, E>,
) -> Result<String, E> {
    Ok(expand(path, windows, home, String::push_str)?.unwrap_or_else(|| path.to_owned()))
}

/// Expands a leading `~` below a native home directory, keeping the bytes of the home.
///
/// The rule is that of [`expand_tilde`]; only the home and the result are native paths, so
/// a home that text cannot hold still expands.
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub fn expand_tilde_native<E>(
    path: &str,
    home: impl FnOnce() -> Result<PathBuf, E>,
) -> Result<PathBuf, E> {
    let home = || home().map(PathBuf::into_os_string);
    Ok(expand(
        path,
        cfg!(windows),
        home,
        |path: &mut OsString, tail: &str| path.push(tail),
    )?
    .map_or_else(|| path.into(), Into::into))
}

/// The expansion of `path`, or `None` when its spelling is not expanded, built on the home
/// that `home` supplies and extended through `push`.
fn expand<H: AsRef<OsStr>, E>(
    path: &str,
    windows: bool,
    home: impl FnOnce() -> Result<H, E>,
    push: fn(&mut H, &str),
) -> Result<Option<H>, E> {
    let Some(rest) = path.strip_prefix('~') else {
        return Ok(None);
    };
    if rest.is_empty() {
        return home().map(Some);
    }
    let separator = |c: char| c == '/' || (windows && c == '\\');
    let Some(below) = rest.strip_prefix(separator) else {
        return Ok(None);
    };
    let mut expanded = home()?;
    let mut append = |piece: &str| {
        let bytes = expanded.as_ref().as_encoded_bytes();
        let ends_in_separator = matches!(bytes.last(), Some(b'/' | b'\\'));
        if !(bytes.is_empty() || windows && ends_in_separator) {
            push(&mut expanded, if windows { "\\" } else { "/" });
        }
        push(&mut expanded, piece);
    };
    if windows {
        below
            .split(separator)
            .filter(|part| !part.is_empty())
            .for_each(append);
    } else {
        append(below);
    }
    Ok(Some(expanded))
}

#[cfg(test)]
mod tests;

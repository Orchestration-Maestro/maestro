//! Home-directory expansion of authored paths.
use crate::join;
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

/// Which separators introduce a path below the home.
#[derive(Clone, Copy, Debug)]
enum Separators {
    /// Only a forward slash introduces the suffix.
    Slash,
    /// Either Windows separator introduces the suffix.
    Windows,
}

impl Separators {
    /// The separators of the compile target.
    const NATIVE: Self = if cfg!(windows) {
        Self::Windows
    } else {
        Self::Slash
    };

    /// Whether `c` separates path components with these separators.
    fn contains(self, c: char) -> bool {
        c == '/' || (matches!(self, Self::Windows) && c == '\\')
    }
}

/// Reads a leading `~` of `path` in the compile target's flavor.
#[must_use]
pub fn parse_tilde(path: &str) -> Tilde<'_> {
    parse(path, Separators::NATIVE)
}

/// Reads a leading `~` of `path` in a supplied flavor.
fn parse(path: &str, separators: Separators) -> Tilde<'_> {
    match path.strip_prefix('~') {
        None => Tilde::Other,
        Some("") => Tilde::Home,
        Some(rest) => rest
            .strip_prefix(|c| separators.contains(c))
            .map_or(Tilde::Other, Tilde::Below),
    }
}

/// Expands a leading `~` below the home directory with a normalizing join.
///
/// An exact `~` is the home directory as supplied. `~` plus a separator joins the rest below
/// it through [`join`], so `.`, `..` and repeated separators resolve and an empty home leaves
/// a relative result. Every other spelling, `~name` included, is returned as written, and the
/// home is read only for the two expanded forms.
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub fn expand_tilde_joined<E>(
    path: &str,
    home: impl FnOnce() -> Result<String, E>,
) -> Result<String, E> {
    join_form(path, parse_tilde(path), home)
}

/// Expands only `~` and `~/` with the compile target's normalizing join.
///
/// Backslash home forms stay as written, even on Windows, without reading `home`.
/// An exact `~` returns the home as supplied; `~/` joins the rest through [`join`].
///
/// # Errors
/// Returns the error `home` returns when the path needs the home directory.
pub fn join_home<E>(path: &str, home: impl FnOnce() -> Result<String, E>) -> Result<String, E> {
    join_form(path, parse(path, Separators::Slash), home)
}

/// Joins a parsed home form, reading the home only when expansion needs it.
fn join_form<E>(
    path: &str,
    form: Tilde<'_>,
    home: impl FnOnce() -> Result<String, E>,
) -> Result<String, E> {
    match form {
        Tilde::Other => Ok(path.to_owned()),
        Tilde::Home => home(),
        Tilde::Below(rest) => Ok(join(&[&home()?, rest])),
    }
}

/// Expands `~` and `~/` by appending the suffix to the home text unchanged.
///
/// An exact `~` returns the supplied home. For `~/`, the leading slash and the rest
/// are appended verbatim, including repeated separators and `..`. A trailing home
/// separator is kept too. Every other spelling, including `~name` and `~\` on
/// Windows, stays as written without reading the home.
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
    match parse(path, Separators::Slash) {
        Tilde::Other => Ok(path.to_owned()),
        Tilde::Home => home(),
        Tilde::Below(rest) => Ok(format!("{}/{rest}", home()?)),
    }
}

/// Expands a leading `~` below a native home directory, keeping the bytes of the home.
///
/// Either separator introduces the rest on Windows; empty components drop and
/// the rest joins with `\`. Other targets accept only `~/` and append the rest
/// without adding a slash when the home ends in one. A home that text cannot hold
/// still expands. Every unsupported spelling stays as written without reading the home.
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
        Separators::NATIVE,
        home,
        |path: &mut OsString, tail: &str| path.push(tail),
    )?
    .map_or_else(|| path.into(), Into::into))
}

/// The concatenating expansion of `path`, or `None` when its spelling is not expanded, built on
/// the home that `home` supplies and extended through `push`.
fn expand<H: AsRef<OsStr>, E>(
    path: &str,
    separators: Separators,
    home: impl FnOnce() -> Result<H, E>,
    push: fn(&mut H, &str),
) -> Result<Option<H>, E> {
    let windows = matches!(separators, Separators::Windows);
    let below = match parse(path, separators) {
        Tilde::Other => return Ok(None),
        Tilde::Home => return home().map(Some),
        Tilde::Below(below) => below,
    };
    let mut expanded = home()?;
    let mut append = |piece: &str| {
        let last = expanded.as_ref().as_encoded_bytes().last().copied();
        let ends_in_separator = last == Some(b'/') || (windows && last == Some(b'\\'));
        if !ends_in_separator {
            push(&mut expanded, if windows { "\\" } else { "/" });
        }
        push(&mut expanded, piece);
    };
    if windows {
        below
            .split(|c| separators.contains(c))
            .filter(|part| !part.is_empty())
            .for_each(append);
    } else {
        append(below);
    }
    Ok(Some(expanded))
}

#[cfg(test)]
mod tests;

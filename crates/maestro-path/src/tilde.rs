//! Home-directory expansion of authored paths.
use crate::join;
use crate::path::{Cwd, posix, win32};
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
    match path.strip_prefix('~') {
        None => Tilde::Other,
        Some("") => Tilde::Home,
        Some(rest) => rest
            .strip_prefix(|c| is_separator(c, windows))
            .map_or(Tilde::Other, Tilde::Below),
    }
}

/// Whether `c` separates path components in the supplied flavor.
fn is_separator(c: char, windows: bool) -> bool {
    c == '/' || (windows && c == '\\')
}

/// What follows `~` when a path is shown below the home.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Rest {
    /// The text after the home exactly as written.
    AsWritten,
    /// The part below the home after its `.` and `..` segments fold.
    Resolved,
}

/// Shows `path` below the directory `home` as `~` and what follows it, in the compile target's
/// flavor.
///
/// `path` and `home` are authored, absolute and compared lexically after their `.` and `..`
/// segments fold. The home itself is `~`; a path below it is `~`, the flavor's separator and
/// the part below. A sibling that shares the home's leading text, a path on another root, a
/// relative path, and an empty, relative or root home leave `path` as written.
#[must_use]
pub fn shorten_home(path: &str, home: &str) -> String {
    shorten_home_in(path, home, cfg!(windows), Rest::AsWritten)
}

/// Shows `path` below `home` as `~` and what follows it, in the compile target's flavor.
#[must_use]
pub fn shorten_home_normalized(path: &str, home: &str) -> String {
    shorten_home_in(path, home, cfg!(windows), Rest::Resolved)
}

/// The rules of [`shorten_home`] and [`shorten_home_normalized`] in a supplied flavor,
/// independently of the compile target.
fn shorten_home_in(path: &str, home: &str, windows: bool, _rest: Rest) -> String {
    let (absolute, relative, separator): (fn(&str) -> bool, RelativeFn, char) = if windows {
        (win32::is_absolute, win32::relative, '\\')
    } else {
        (posix::is_absolute, posix::relative, '/')
    };
    if !absolute(home) || !absolute(path) {
        return path.to_owned();
    }
    // Both operands are absolute, so the directory below is never consulted.
    let cwd = Cwd {
        current: home,
        drive_directories: &[],
    };
    // A root is its own parent, and a root home would abbreviate every absolute path.
    if relative(home, &format!("{home}{separator}.."), &cwd).is_empty() {
        return path.to_owned();
    }
    let below = relative(home, path, &cwd);
    if below.is_empty() {
        return "~".to_owned();
    }
    let outside = below == ".." || below.starts_with(&format!("..{separator}")) || absolute(&below);
    if outside {
        path.to_owned()
    } else {
        format!("~{separator}{below}")
    }
}

/// The signature of a flavor's `relative`.
type RelativeFn = for<'a> fn(&str, &str, &Cwd<'a>) -> String;

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
    match parse_tilde(path) {
        Tilde::Other => Ok(path.to_owned()),
        Tilde::Home => home(),
        Tilde::Below(rest) => Ok(join(&[&home()?, rest])),
    }
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

/// The rule of [`expand_tilde`] in a supplied flavor, independently of the compile target.
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

/// The concatenating expansion of `path`, or `None` when its spelling is not expanded, built on
/// the home that `home` supplies and extended through `push`.
fn expand<H: AsRef<OsStr>, E>(
    path: &str,
    windows: bool,
    home: impl FnOnce() -> Result<H, E>,
    push: fn(&mut H, &str),
) -> Result<Option<H>, E> {
    let below = match parse(path, windows) {
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
            .split(|c| is_separator(c, windows))
            .filter(|part| !part.is_empty())
            .for_each(append);
    } else {
        append(below);
    }
    Ok(Some(expanded))
}

#[cfg(test)]
mod tests;

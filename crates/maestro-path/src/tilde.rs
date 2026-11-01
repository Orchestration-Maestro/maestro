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

/// Shows `path` below the directory `home` as `~` and the text after the home as written.
///
/// `path` and `home` are authored and absolute. The home itself is `~`; a path that starts
/// with the home's text, ignoring separators at its end, and continues with a separator is `~`
/// and the rest exactly as written, so `.`, `..` and doubled or trailing separators stay. A
/// sibling that shares the home's leading text, a path on another root, a relative path, and an
/// empty, relative or root home leave `path` as written. The compile target's flavor decides
/// what a separator and an absolute path are.
#[must_use]
pub fn shorten_home(path: &str, home: &str) -> String {
    shorten_home_in(path, home, cfg!(windows), Rest::AsWritten)
}

/// Shows `path` below the directory `home` as `~` and the part below it, normalized.
///
/// `path` and `home` are authored, absolute and compared lexically after their `.` and `..`
/// segments fold. The home itself is `~`; a path below it is `~`, the compile target's separator
/// and the part below. The cases [`shorten_home`] leaves as written stay as written.
#[must_use]
pub fn shorten_home_normalized(path: &str, home: &str) -> String {
    shorten_home_in(path, home, cfg!(windows), Rest::Resolved)
}

/// The rules of [`shorten_home`] and [`shorten_home_normalized`] in a supplied flavor,
/// independently of the compile target.
fn shorten_home_in(path: &str, home: &str, windows: bool, rest: Rest) -> String {
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
    if rest == Rest::AsWritten {
        return below_as_written(path, home, windows);
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

/// `~` and the text of `path` after the text of `home` when `path` continues it at a component
/// boundary, and `path` otherwise.
fn below_as_written(path: &str, home: &str, windows: bool) -> String {
    let home = home.trim_end_matches(|c| is_separator(c, windows));
    match path.strip_prefix(home) {
        Some(rest) if rest.is_empty() || rest.starts_with(|c| is_separator(c, windows)) => {
            format!("~{rest}")
        }
        _ => path.to_owned(),
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

//! POSIX flavor: `/` is the only separator and every comparison is exact.

use super::Cwd;
use super::segments::{base_name, climb_and_descend, reduce};

/// The path separator.
pub const SEP: char = '/';

/// Report whether `path` starts at the root.
///
/// # Examples
///
/// ```
/// use maestro_path::posix;
///
/// assert!(posix::is_absolute("/usr/bin"));
/// assert!(!posix::is_absolute("usr/bin"));
/// assert!(!posix::is_absolute("\\usr"));
/// ```
#[must_use]
pub fn is_absolute(path: &str) -> bool {
    path.starts_with(SEP)
}

/// Collapse repeated separators and resolve `.` and `..` segments.
///
/// An absolute path stays at the root when `..` climbs past it; a relative path
/// keeps its leading `..` segments. A trailing separator is kept, and an empty
/// result is `.` (or `./` when the input ended in a separator).
///
/// # Examples
///
/// ```
/// use maestro_path::posix;
///
/// assert_eq!(posix::normalize("/a//b/../c/"), "/a/c/");
/// assert_eq!(posix::normalize("../a/./b"), "../a/b");
/// assert_eq!(posix::normalize("/.."), "/");
/// assert_eq!(posix::normalize(""), ".");
/// ```
#[must_use]
pub fn normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let absolute = is_absolute(path);
    let trailing = path.ends_with(SEP);
    let mut normalized = reduce(path.split(SEP), !absolute).join("/");
    if normalized.is_empty() {
        let root = if absolute {
            "/"
        } else if trailing {
            "./"
        } else {
            "."
        };
        return root.to_owned();
    }
    if trailing {
        normalized.push(SEP);
    }
    if absolute {
        normalized.insert(0, SEP);
    }
    normalized
}

/// Concatenate the non-empty operands with `/` and normalize the result.
///
/// Unlike [`resolve`], a later operand that looks absolute does not discard the
/// ones before it. No operands, or only empty ones, give `.`.
///
/// # Examples
///
/// ```
/// use maestro_path::posix;
///
/// assert_eq!(posix::join(&["/a", "b", "../c"]), "/a/c");
/// assert_eq!(posix::join(&["a", "/b"]), "a/b");
/// assert_eq!(posix::join(&["", ""]), ".");
/// ```
#[must_use]
pub fn join(paths: &[&str]) -> String {
    let operands: Vec<&str> = paths
        .iter()
        .copied()
        .filter(|path| !path.is_empty())
        .collect();
    if operands.is_empty() {
        return ".".to_owned();
    }
    normalize(&operands.join("/"))
}

/// Remove the last non-empty component, keeping the rest of the path as written.
///
/// Separators before the removed component stay, so `a//b` gives `a/`. A path
/// without a directory part gives `/` when rooted and `.` otherwise, except
/// that `//x` keeps its double root.
///
/// # Examples
///
/// ```
/// use maestro_path::posix;
///
/// assert_eq!(posix::dirname("/a/b/c"), "/a/b");
/// assert_eq!(posix::dirname("a//b/"), "a/");
/// assert_eq!(posix::dirname("/a"), "/");
/// assert_eq!(posix::dirname("a"), ".");
/// ```
#[must_use]
pub fn dirname(path: &str) -> String {
    let Some(first) = path.chars().next() else {
        return ".".to_owned();
    };
    let skip = first.len_utf8();
    let rooted = is_absolute(path);
    let parent_end = path[skip..]
        .trim_end_matches(SEP)
        .rfind(SEP)
        .map(|index| skip + index);
    match parent_end {
        None if rooted => "/".to_owned(),
        None => ".".to_owned(),
        Some(1) if rooted => "//".to_owned(),
        Some(end) => path[..end].to_owned(),
    }
}

/// Return the last component, optionally without a literal `suffix`.
///
/// Trailing separators are ignored. The suffix is removed only from the end of
/// that component, case-sensitively, and never when the component would become
/// empty; a suffix equal to the whole path gives an empty name.
///
/// # Examples
///
/// ```
/// use maestro_path::posix;
///
/// assert_eq!(posix::basename("/a/b.txt", None), "b.txt");
/// assert_eq!(posix::basename("/a/b.txt/", Some(".txt")), "b");
/// assert_eq!(posix::basename("/a/b.txt", Some("b.txt")), "b.txt");
/// assert_eq!(posix::basename("/", None), "");
/// ```
#[must_use]
pub fn basename(path: &str, suffix: Option<&str>) -> String {
    base_name(path, 0, suffix, |c| c == SEP).to_owned()
}

/// The parts of a path as written, borrowed from it.
///
/// For `/home/user/file.txt`, `root` is `/`, `dir` is `/home/user`, `base` is
/// `file.txt`, `name` is `file` and `ext` is `.txt`. A part that is absent is
/// empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedPath<'a> {
    /// The leading `/` of an absolute path.
    pub root: &'a str,
    /// Everything before the final component, with its separators as written.
    pub dir: &'a str,
    /// The final component.
    pub base: &'a str,
    /// The last dot of the final component and the text after it.
    pub ext: &'a str,
    /// The final component without its extension.
    pub name: &'a str,
}

/// Split a path into root, directory, base name, extension and stem.
///
/// Trailing separators are ignored. A leading dot starts no extension, so
/// `.profile` has none, and neither does `..`.
///
/// # Examples
///
/// ```
/// use maestro_path::posix::{ParsedPath, parse};
///
/// assert_eq!(
///     parse("/home/user/file.tar.gz"),
///     ParsedPath {
///         root: "/",
///         dir: "/home/user",
///         base: "file.tar.gz",
///         ext: ".gz",
///         name: "file.tar",
///     }
/// );
/// assert_eq!(parse(".profile").ext, "");
/// ```
#[must_use]
pub fn parse(path: &str) -> ParsedPath<'_> {
    let root = if is_absolute(path) { &path[..1] } else { "" };
    let trimmed = path[root.len()..].trim_end_matches(SEP);
    let (dir, base) = match trimmed.rfind(SEP) {
        Some(separator) => (&path[..root.len() + separator], &trimmed[separator + 1..]),
        None => (root, trimmed),
    };
    let (name, ext) = match base.rfind('.') {
        Some(dot) if dot > 0 && base != ".." => base.split_at(dot),
        _ => (base, ""),
    };
    ParsedPath {
        root,
        dir,
        base,
        ext,
        name,
    }
}

/// Resolve `paths` from right to left into one path.
///
/// Operands are applied until one is absolute; if none is, `cwd.current` is the
/// base. The result is absolute unless that base is not (an empty
/// `cwd.current`, for example), in which case it is relative, or `.` when
/// nothing is left. Empty operands are ignored. With no operands, or only an
/// empty one or `.`, a rooted working directory is returned exactly as supplied.
///
/// # Examples
///
/// ```
/// use maestro_path::{Cwd, posix};
///
/// let cwd = Cwd { current: "/work/home", drive_directories: &[] };
/// assert_eq!(posix::resolve(&["docs", "../notes"], &cwd), "/work/home/notes");
/// assert_eq!(posix::resolve(&["/a", "b", "/c"], &cwd), "/c");
/// assert_eq!(posix::resolve(&[], &cwd), "/work/home");
///
/// let unanchored = Cwd { current: "", drive_directories: &[] };
/// assert_eq!(posix::resolve(&[], &unanchored), ".");
/// ```
#[must_use]
pub fn resolve(paths: &[&str], cwd: &Cwd<'_>) -> String {
    if is_absolute(cwd.current) && matches!(paths, [] | ["" | "."]) {
        return cwd.current.to_owned();
    }
    let mut operands = Vec::new();
    let mut absolute = false;
    for path in paths.iter().rev().filter(|path| !path.is_empty()) {
        operands.push(*path);
        if is_absolute(path) {
            absolute = true;
            break;
        }
    }
    if !absolute {
        operands.push(cwd.current);
        absolute = is_absolute(cwd.current);
    }
    let resolved = reduce(
        operands.iter().rev().flat_map(|path| path.split(SEP)),
        !absolute,
    )
    .join("/");
    match (absolute, resolved.is_empty()) {
        (true, _) => format!("/{resolved}"),
        (false, true) => ".".to_owned(),
        (false, false) => resolved,
    }
}

/// Find the path that leads from `from` to `to`, both resolved against `cwd`,
/// which should be absolute.
///
/// Both ends are normalized first, so `.` and `..` that come from `cwd` are
/// folded. Components are compared whole, so `/a/bc` is not below `/a/b`. Equal
/// locations give an empty string.
///
/// # Examples
///
/// ```
/// use maestro_path::{Cwd, posix};
///
/// let cwd = Cwd { current: "/work/home", drive_directories: &[] };
/// assert_eq!(posix::relative("/data/a/b", "/data/c", &cwd), "../../c");
/// assert_eq!(posix::relative("/data/a", "/data/a/b/c", &cwd), "b/c");
/// assert_eq!(posix::relative("/data/b", "/data/bc", &cwd), "../bc");
///
/// let dotted = Cwd { current: "/work/a/../b/.", drive_directories: &[] };
/// assert_eq!(posix::relative("", "c", &dotted), "c");
/// ```
#[must_use]
pub fn relative(from: &str, to: &str, cwd: &Cwd<'_>) -> String {
    let from = resolve(&[from], cwd);
    let to = resolve(&[to], cwd);
    let from = reduce(from.split(SEP), !is_absolute(&from));
    let to = reduce(to.split(SEP), !is_absolute(&to));
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    climb_and_descend(from.len() - shared, &to[shared..], "/")
}

//! Windows flavor: `\` and `/` both separate, drives and UNC roots exist, and
//! names compare without regard to case.

use std::borrow::Cow;

mod relative;
mod resolve;

pub use relative::relative;
pub use resolve::resolve;

use super::Root;
use super::segments::{base_name, reduce};

/// The path separator.
pub const SEP: char = '\\';

/// Names Windows reserves for devices, compared without regard to ASCII case.
const RESERVED: [&str; 28] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "COM¹", "COM²",
    "COM³", "LPT¹", "LPT²", "LPT³",
];

/// Report whether `c` separates path components.
pub(crate) fn is_separator(c: char) -> bool {
    c == '/' || c == SEP
}

/// Report whether two names are equal when Unicode case is ignored.
fn same_name(a: &str, b: &str) -> bool {
    a == b || a.to_lowercase() == b.to_lowercase()
}

/// Report whether `name` is a reserved device name.
fn is_reserved(name: &str) -> bool {
    RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(name))
}

/// Report whether a UNC-shaped server component is really a device namespace.
fn is_namespace(server: &str) -> bool {
    matches!(server, "." | "?")
}

/// The text before the first colon of `path`, which names a device when reserved.
///
/// Without a colon the final character is dropped instead, so `CON.` counts as
/// the device `CON`. A final character outside the Basic Multilingual Plane
/// leaves no candidate: `CON😀` is not a device name.
fn device_candidate(path: &str) -> &str {
    match path.find(':') {
        Some(colon) => &path[..colon],
        None => match path.char_indices().next_back() {
            Some((last, c)) if c.len_utf16() == 1 => &path[..last],
            _ => "",
        },
    }
}

/// How a path starts: the device kept in front of the normalized tail, where
/// the tail begins and whether the root fixes the location.
struct Prefix<'a> {
    /// The device or UNC root as it will be written, when the path has one.
    device: Option<Cow<'a, str>>,
    /// Byte offset of the first unnormalized component.
    tail_start: usize,
    /// Whether the root makes the path independent of the current directory.
    absolute: bool,
}

impl<'a> Prefix<'a> {
    /// Interpret `root` the way normalization does.
    fn of(path: &'a str, root: Root<'a>) -> Self {
        match root {
            Root::Rooted => Self {
                device: None,
                tail_start: 0,
                absolute: true,
            },
            Root::Drive { prefix, absolute } => Self {
                device: Some(prefix.into()),
                tail_start: prefix.len() + usize::from(absolute),
                absolute,
            },
            Root::Unc { server, .. } if is_namespace(server) => Self::namespace(path, server),
            Root::Unc { server, share, end } => Self {
                device: Some(format!(r"\\{server}\{share}").into()),
                tail_start: end,
                absolute: true,
            },
            Root::Relative => Self::reserved_device(path),
        }
    }

    /// A `\\.\` or `\\?\` namespace, which keeps a reserved device name written
    /// after it together with its colon.
    fn namespace(path: &'a str, server: &str) -> Self {
        let reserved = path
            .find(':')
            .and_then(|colon| Some((path.get(4..colon)?, colon)))
            .filter(|(name, _)| is_reserved(name));
        match reserved {
            Some((name, colon)) => Self {
                device: Some(format!(r"\\{server}\{name}:").into()),
                tail_start: colon + 1,
                absolute: true,
            },
            None => Self {
                device: Some(format!(r"\\{server}").into()),
                tail_start: 4,
                absolute: true,
            },
        }
    }

    /// A relative path that starts with a reserved device name and a colon.
    fn reserved_device(path: &'a str) -> Self {
        let device = path
            .find(':')
            .filter(|colon| *colon > 0 && is_reserved(&path[..*colon]));
        Self {
            device: device.map(|colon| path[..=colon].into()),
            tail_start: device.map_or(0, |colon| colon + 1),
            absolute: false,
        }
    }
}

/// Report whether a relative, deviceless path would read as a drive or device
/// after normalization: a colon that ends a component, or a tail that opens
/// with a drive letter and colon.
fn reads_as_drive(path: &str, tail: &str) -> bool {
    let tail = tail.as_bytes();
    (tail.len() >= 2 && tail[0].is_ascii_alphabetic() && tail[1] == b':')
        || path
            .match_indices(':')
            .any(|(colon, _)| path[colon + 1..].chars().next().is_none_or(is_separator))
}

/// Collapse separators and resolve `.` and `..` segments, writing every
/// separator as `\`.
///
/// A drive, UNC or device root is kept in front. A relative path that could
/// read as a drive or reserved device name after normalization gets a leading
/// `.\` so it stays relative. A trailing separator is kept; an empty result is
/// `.`.
///
/// # Examples
///
/// ```
/// use maestro_path::win32;
///
/// assert_eq!(win32::normalize("C:/a//b/../c/"), "C:\\a\\c\\");
/// assert_eq!(win32::normalize("\\\\server\\share"), "\\\\server\\share\\");
/// assert_eq!(win32::normalize("ab:"), ".\\ab:");
/// assert_eq!(win32::normalize("CON:x"), ".\\CON:x");
/// ```
#[must_use]
pub fn normalize(path: &str) -> String {
    let mut chars = path.chars();
    match (chars.next(), chars.next()) {
        (None, _) => return ".".to_owned(),
        (Some(only), None) => {
            return if is_separator(only) {
                "\\".to_owned()
            } else {
                path.to_owned()
            };
        }
        _ => {}
    }
    let root = Root::parse(path);
    if let Root::Unc { server, share, end } = root
        && end == path.len()
        && !is_namespace(server)
    {
        return format!(r"\\{server}\{share}\");
    }
    let prefix = Prefix::of(path, root);
    let mut tail = reduce(
        path[prefix.tail_start..].split(is_separator),
        !prefix.absolute,
    )
    .join("\\");
    if tail.is_empty() && !prefix.absolute {
        tail.push('.');
    }
    if !tail.is_empty() && path.ends_with(is_separator) {
        tail.push(SEP);
    }
    if prefix.device.is_none() && !prefix.absolute && reads_as_drive(path, &tail) {
        return format!(".\\{tail}");
    }
    let device = prefix.device.as_deref();
    if is_reserved(device_candidate(path)) {
        return format!(".\\{}{tail}", device.unwrap_or_default());
    }
    match (device, prefix.absolute) {
        (None, true) => format!("\\{tail}"),
        (None, false) => tail,
        (Some(device), true) => format!("{device}\\{tail}"),
        (Some(device), false) => format!("{device}{tail}"),
    }
}

/// Drop the extra leading separators that joining would turn into a UNC root.
///
/// The first operand keeps its separators only when it opens with exactly two
/// of them followed by another character, which states the intent to name a UNC
/// path. Any other run of two or more at the start of `joined` becomes one.
fn collapse_leading_separators(first: &str, joined: String) -> String {
    let mut chars = first.chars();
    let states_unc = matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(a), Some(b), Some(c)) if is_separator(a) && is_separator(b) && !is_separator(c)
    );
    let run = joined.chars().take_while(|c| is_separator(*c)).count();
    if states_unc || run < 2 {
        return joined;
    }
    format!("\\{}", &joined[run..])
}

/// Concatenate the non-empty operands with `\` and normalize the result.
///
/// Normalization is skipped when any joined component has a reserved device
/// name before a colon: the text is kept as written, apart from `/` becoming
/// `\`. Operands never start a new root, so a later `C:\x` does not discard
/// what precedes it. No operands, or only empty ones, give `.`.
///
/// # Examples
///
/// ```
/// use maestro_path::win32;
///
/// assert_eq!(win32::join(&["C:\\a", "..", "b"]), "C:\\b");
/// assert_eq!(win32::join(&["//server", "share"]), "\\\\server\\share\\");
/// assert_eq!(win32::join(&["C:\\a", "D:\\b"]), "C:\\a\\D:\\b");
/// ```
#[must_use]
pub fn join(paths: &[&str]) -> String {
    let operands: Vec<&str> = paths
        .iter()
        .copied()
        .filter(|path| !path.is_empty())
        .collect();
    let Some(first) = operands.first() else {
        return ".".to_owned();
    };
    let joined = collapse_leading_separators(first, operands.join("\\"));
    let names_device = joined.split(SEP).any(|component| {
        component
            .split_once(':')
            .is_some_and(|(name, _)| is_reserved(name))
    });
    if names_device {
        return joined.replace('/', "\\");
    }
    normalize(&joined)
}

/// Remove the last non-empty component, keeping the rest of the path as written.
///
/// The root is kept unnormalized: a path that is only a UNC root, or a device
/// namespace such as `\\.\COM1`, is returned whole, and a path with no
/// directory part gives its root (`C:\`, `C:`, `\`) or `.`.
///
/// # Examples
///
/// ```
/// use maestro_path::win32;
///
/// assert_eq!(win32::dirname("C:\\a\\b"), "C:\\a");
/// assert_eq!(win32::dirname("C:\\a"), "C:\\");
/// assert_eq!(win32::dirname("\\\\server\\share\\a"), "\\\\server\\share\\");
/// assert_eq!(win32::dirname("a"), ".");
/// ```
#[must_use]
pub fn dirname(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let root_end = match Root::parse(path) {
        Root::Relative => None,
        Root::Rooted => Some(1),
        Root::Drive { prefix, absolute } => Some(prefix.len() + usize::from(absolute)),
        Root::Unc { end, .. } if end == path.len() => return path.to_owned(),
        Root::Unc { end, .. } => Some(end + 1),
    };
    let offset = root_end.unwrap_or_default();
    let parent_end = path[offset..]
        .trim_end_matches(is_separator)
        .rfind(is_separator)
        .map(|index| offset + index);
    match parent_end.or(root_end) {
        Some(end) => path[..end].to_owned(),
        None => ".".to_owned(),
    }
}

/// Return the last component, optionally without a literal `suffix`.
///
/// A drive prefix (`C:`) is not part of the name, and trailing separators are
/// ignored. The suffix is removed only from the end of the component,
/// case-sensitively, and never when the component would become empty; a suffix
/// equal to the whole path gives an empty name.
///
/// # Examples
///
/// ```
/// use maestro_path::win32;
///
/// assert_eq!(win32::basename("C:\\dir\\file.txt", None), "file.txt");
/// assert_eq!(win32::basename("C:\\dir\\file.txt\\", Some(".txt")), "file");
/// assert_eq!(win32::basename("C:", None), "");
/// ```
#[must_use]
pub fn basename(path: &str, suffix: Option<&str>) -> String {
    let start = match Root::parse(path) {
        Root::Drive { prefix, .. } => prefix.len(),
        _ => 0,
    };
    base_name(path, start, suffix, is_separator).to_owned()
}

/// Report whether `path` starts at a root: a separator, a UNC root, or a drive
/// letter and colon followed by a separator.
///
/// # Examples
///
/// ```
/// use maestro_path::win32;
///
/// assert!(win32::is_absolute("C:\\Users"));
/// assert!(win32::is_absolute("\\\\server\\share"));
/// assert!(!win32::is_absolute("C:Users"));
/// ```
#[must_use]
pub fn is_absolute(path: &str) -> bool {
    matches!(
        Root::parse(path),
        Root::Rooted | Root::Unc { .. } | Root::Drive { absolute: true, .. }
    )
}

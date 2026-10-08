//! The path between two Windows locations.

use super::{resolve, same_name};
use crate::path::segments::climb_and_descend;
use crate::path::{Cwd, Root};

/// The non-empty components of a resolved path, root components included.
fn components(resolved: &str) -> Vec<&str> {
    resolved
        .split('\\')
        .filter(|part| !part.is_empty())
        .collect()
}

/// Report whether two resolved paths start at the same root: the same drive,
/// the same UNC share, or none.
///
/// Component text cannot tell a drive from a UNC server, and a parent step
/// cannot leave a drive or a share, so roots compare by kind before components.
fn same_root(from: &str, to: &str) -> bool {
    match (Root::parse(from), Root::parse(to)) {
        (Root::Drive { prefix: a, .. }, Root::Drive { prefix: b, .. }) => same_name(a, b),
        (
            Root::Unc {
                server: a_server,
                share: a_share,
                ..
            },
            Root::Unc {
                server: b_server,
                share: b_share,
                ..
            },
        ) => same_name(a_server, b_server) && same_name(a_share, b_share),
        (Root::Relative, Root::Relative) | (Root::Rooted, Root::Rooted) => true,
        _ => false,
    }
}

/// Find the path that leads from `from` to `to`, both resolved against `cwd`,
/// which should be absolute.
///
/// Whole components are compared with Unicode lowercase equality and the
/// destination keeps the spelling it was resolved with. Two drives, two UNC
/// shares, a drive and a UNC share, or paths with no shared leading component
/// have no relative path, so the resolved destination is returned. Equal
/// locations give an empty string.
///
/// # Examples
///
/// ```
/// use maestro_path::{Cwd, win32};
///
/// let cwd = Cwd { current: "C:\\work", drive_directories: &[] };
/// assert_eq!(win32::relative("C:\\a\\b", "c:\\A\\c\\D", &cwd), "..\\c\\D");
/// assert_eq!(win32::relative("C:\\a", "D:\\b", &cwd), "D:\\b");
/// assert_eq!(win32::relative("\\\\one\\share", "\\\\two\\share", &cwd), "\\\\two\\share\\");
/// assert_eq!(win32::relative("\\\\one\\a\\x", "\\\\one\\b\\x", &cwd), "\\\\one\\b\\x");
/// ```
#[must_use]
pub fn relative(from: &str, to: &str, cwd: &Cwd<'_>) -> String {
    let from = resolve(&[from], cwd);
    let to = resolve(&[to], cwd);
    if !same_root(&from, &to) {
        return to;
    }
    let from_parts = components(&from);
    let to_parts = components(&to);
    let shared = from_parts
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| same_name(a, b))
        .count();
    if shared == 0 && !from_parts.is_empty() && !to_parts.is_empty() {
        return to;
    }
    climb_and_descend(from_parts.len() - shared, &to_parts[shared..], "\\")
}

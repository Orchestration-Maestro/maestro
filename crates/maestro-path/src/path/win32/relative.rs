//! The path between two Windows locations.

use super::resolve::{Location, locate};
use super::same_name;
use crate::path::Cwd;
use crate::path::segments::climb_and_descend;

/// Report whether both paths start at the same root: the same drive, the same
/// UNC share, the same namespace, or none.
///
/// Component text cannot tell a drive from a UNC server, and a parent step
/// cannot leave a drive, a share or a namespace, so roots compare before
/// components.
fn same_root(from: &Location<'_>, to: &Location<'_>) -> bool {
    match (&from.device, &to.device) {
        (Some(a), Some(b)) => same_name(a, b),
        (None, None) => from.absolute == to.absolute,
        _ => false,
    }
}

/// Find the path that leads from `from` to `to`, both resolved against `cwd`,
/// which should be absolute.
///
/// Both ends are normalized first, so `.` and `..` that come from `cwd` are
/// folded. Whole components are compared with Unicode lowercase equality and
/// the destination keeps the spelling it was resolved with. Paths that start
/// at different roots (another drive, UNC share or device namespace, or a root
/// of another kind) have no relative path, and neither have two paths without
/// a drive, share or namespace whose first components differ: the normalized
/// destination is returned. Equal locations give an empty string.
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
/// assert_eq!(win32::relative("\\\\?\\a\\x", "\\\\?\\b\\y", &cwd), "..\\..\\b\\y");
/// ```
#[must_use]
pub fn relative(from: &str, to: &str, cwd: &Cwd<'_>) -> String {
    let (from, to) = (locate(&[from], cwd), locate(&[to], cwd));
    if !same_root(&from, &to) {
        return to.spell();
    }
    let shared = from
        .parts
        .iter()
        .zip(&to.parts)
        .take_while(|(a, b)| same_name(a, b))
        .count();
    let rootless = from.device.is_none();
    if shared == 0 && rootless && !from.parts.is_empty() && !to.parts.is_empty() {
        return to.spell();
    }
    climb_and_descend(from.parts.len() - shared, &to.parts[shared..], "\\")
}

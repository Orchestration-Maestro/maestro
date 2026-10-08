//! The path between two Windows locations.

use super::resolve::{Location, locate};
use super::same_name;
use crate::path::Cwd;
use crate::path::segments::climb_and_descend;

/// Report whether both paths start at the same root: the same drive, the same
/// UNC share, the same namespace, or none, and both absolute or both not.
///
/// Component text cannot tell a drive from a UNC server, a parent step cannot
/// leave a drive, a share or a namespace, and `D:foo` is not below `D:\`, so
/// roots compare before components.
fn same_root(from: &Location<'_>, to: &Location<'_>) -> bool {
    from.absolute == to.absolute
        && match (&from.device, &to.device) {
            (Some(a), Some(b)) => same_name(a, b),
            (None, None) => true,
            _ => false,
        }
}

/// Find the path that leads from `from` to `to`, both resolved against `cwd`,
/// which should be absolute.
///
/// Both ends are normalized first, so `.` and `..` that come from `cwd` are
/// folded. Whole components are compared with Unicode lowercase equality and
/// the destination keeps the spelling it was resolved with. Paths that start
/// at different roots (another drive, UNC share or device namespace, a root of
/// another kind, or an absolute path against a relative one) have no shared
/// comparison base. A matching named root supplies a base only when anchored;
/// otherwise, nonempty tails must share a complete first component. Without a
/// base the normalized destination is returned. Empty tails permit component
/// traversal, and equal locations give an empty string.
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
///
/// let share = Cwd { current: "\\\\srv\\share\\work", drive_directories: &[] };
/// assert_eq!(win32::relative("D:\\foo", "D:foo", &share), "D:foo");
/// assert_eq!(win32::relative("D:a", "D:b", &share), "D:b");
/// ```
#[must_use]
pub fn relative(from: &str, to: &str, cwd: &Cwd<'_>) -> String {
    let (from, to) = (locate(&[from], cwd), locate(&[to], cwd));
    match shared_base(&from, &to) {
        Some(shared) => climb_and_descend(from.parts.len() - shared, &to.parts[shared..], "\\"),
        None => to.spell(),
    }
}

/// Count shared components when the resolved roots and tails permit traversal.
fn shared_base(from: &Location<'_>, to: &Location<'_>) -> Option<usize> {
    if !same_root(from, to) {
        return None;
    }
    let shared = from
        .parts
        .iter()
        .zip(&to.parts)
        .take_while(|(a, b)| same_name(a, b))
        .count();
    let anchored_root = from.absolute && from.device.is_some();
    if shared == 0 && !anchored_root && !from.parts.is_empty() && !to.parts.is_empty() {
        None
    } else {
        Some(shared)
    }
}

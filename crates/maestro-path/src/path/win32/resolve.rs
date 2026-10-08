//! Resolution of Windows operands against an explicit working-directory snapshot.

use std::borrow::Cow;

use super::{is_namespace, is_separator, same_name};
use crate::path::segments::reduce;
use crate::path::{Cwd, Root};

/// The operands consumed so far, from the last one backwards.
#[derive(Default)]
struct Resolution<'a> {
    /// The drive or UNC root every applicable operand shares, empty until one names it.
    device: Cow<'a, str>,
    /// The text after each operand's root, the last operand first.
    tails: Vec<&'a str>,
    /// Whether an operand starts at a root; the operands before it add only a device.
    absolute: bool,
}

impl<'a> Resolution<'a> {
    /// Apply one non-empty operand and report whether the resolution is complete.
    ///
    /// An operand on another device than the one already chosen is skipped.
    fn consume(&mut self, path: &'a str) -> bool {
        let (device, tail, absolute) = match Root::parse(path) {
            Root::Relative => (None, path, false),
            Root::Rooted => (None, path, true),
            Root::Drive { prefix, absolute } => (
                Some(Cow::Borrowed(prefix)),
                &path[prefix.len() + usize::from(absolute)..],
                absolute,
            ),
            Root::Unc { server, .. } if is_namespace(server) => {
                (Some(format!(r"\\{server}").into()), &path[4..], true)
            }
            Root::Unc { server, share, end } => (
                Some(format!(r"\\{server}\{share}").into()),
                &path[end..],
                true,
            ),
        };
        if let Some(device) = device {
            if self.device.is_empty() {
                self.device = device;
            } else if !same_name(&device, &self.device) {
                return false;
            }
        }
        if self.absolute {
            return !self.device.is_empty();
        }
        self.tails.push(tail);
        self.absolute = absolute;
        absolute && !self.device.is_empty()
    }

    /// Continue from the directory of the chosen drive when no operand was absolute.
    fn consume_drive_directory(&mut self, cwd: &Cwd<'a>) {
        let letter = self.device.chars().next().unwrap_or_default();
        let directory = cwd
            .drive_directories
            .iter()
            .find(|(drive, directory)| drive.eq_ignore_ascii_case(&letter) && !directory.is_empty())
            .map_or(cwd.current, |(_, directory)| *directory);
        let (leading, rest) = directory
            .char_indices()
            .nth(2)
            .map_or((directory, ""), |(end, _)| directory.split_at(end));
        if !same_name(leading, &self.device) && rest.starts_with('\\') {
            self.absolute = true;
        } else {
            self.consume(directory);
        }
    }

    /// Write the resolved path with every separator as `\`.
    fn finish(self) -> String {
        let tail = reduce(
            self.tails
                .iter()
                .rev()
                .flat_map(|tail| tail.split(is_separator)),
            !self.absolute,
        )
        .join("\\");
        if self.absolute {
            return format!("{}\\{tail}", self.device);
        }
        let resolved = format!("{}{tail}", self.device);
        if resolved.is_empty() {
            ".".to_owned()
        } else {
            resolved
        }
    }
}

/// Resolve `paths` from right to left into one path, using `cwd` for the part
/// no operand supplies.
///
/// Operands on a different device than the one the rightmost drive or UNC root
/// names are ignored. A drive-relative operand such as `C:notes` continues from
/// the entry of `cwd.drive_directories` for that drive, or from `cwd.current`
/// when the entry is missing or empty; a directory that starts with another
/// drive and a backslash is replaced by the root of the drive. A rooted operand
/// such as `\x` takes its drive or UNC share from `cwd.current`. With no
/// operands, `cwd.current` is returned with `/` written as `\`; so it is for a
/// single operand that is empty or `.` when `cwd.current` starts with a
/// separator.
///
/// # Examples
///
/// ```
/// use maestro_path::{Cwd, win32};
///
/// let cwd = Cwd { current: "D:\\work", drive_directories: &[('C', "C:\\Users")] };
/// assert_eq!(win32::resolve(&["notes", "..\\a"], &cwd), "D:\\work\\a");
/// assert_eq!(win32::resolve(&["C:notes"], &cwd), "C:\\Users\\notes");
/// assert_eq!(win32::resolve(&["C:\\a", "D:\\b", "C:c"], &cwd), "C:\\a\\c");
/// assert_eq!(win32::resolve(&["\\x"], &cwd), "D:\\x");
/// assert_eq!(win32::resolve(&[], &cwd), "D:\\work");
/// ```
#[must_use]
pub fn resolve(paths: &[&str], cwd: &Cwd<'_>) -> String {
    let mut resolution = Resolution::default();
    let complete = paths
        .iter()
        .rev()
        .filter(|path| !path.is_empty())
        .any(|path| resolution.consume(path));
    if !complete {
        if resolution.device.is_empty() {
            let current_only = matches!(paths, ["" | "."]) && cwd.current.starts_with(is_separator);
            if paths.is_empty() || current_only {
                return cwd.current.replace('/', "\\");
            }
            resolution.consume(cwd.current);
        } else {
            resolution.consume_drive_directory(cwd);
        }
    }
    resolution.finish()
}

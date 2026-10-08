//! Shared inputs of the two path flavors.

pub mod posix;
mod segments;
pub mod win32;

/// Working-directory snapshot supplied by the caller, never read from the process.
///
/// `current` is spelled in the flavor it is passed to. The Windows flavor also
/// consults `drive_directories`, one entry per uppercase drive letter, when a
/// drive-relative path such as `C:notes` needs that drive's own directory; the
/// POSIX flavor ignores them. An empty directory counts as absent.
#[derive(Clone, Copy, Debug)]
pub struct Cwd<'a> {
    /// The current working directory.
    pub current: &'a str,
    /// The current directory of each drive, keyed by its uppercase letter.
    pub drive_directories: &'a [(char, &'a str)],
}

/// The raw root of a Windows path, borrowing the authored spelling.
///
/// Normalization, resolution and directory splitting read the same root
/// differently (device namespaces and unmatched UNC prefixes in particular),
/// so the recognizer keeps the pieces and leaves their meaning to each caller.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Root<'a> {
    /// No root: a relative path.
    Relative,
    /// A leading separator that does not open a complete UNC root.
    Rooted,
    /// A drive letter and colon, followed by a separator when `absolute`.
    Drive {
        /// The letter and colon as written.
        prefix: &'a str,
        /// Whether a separator follows the colon.
        absolute: bool,
    },
    /// Two separators, a server and a share, as in `\\server\share`; the
    /// `\\.\device` and `\\?\device` namespaces have the same shape.
    Unc {
        /// The first component, as written.
        server: &'a str,
        /// The second component, as written.
        share: &'a str,
        /// Byte offset just after the share.
        end: usize,
    },
}

impl<'a> Root<'a> {
    /// Recognize the root at the start of `path`.
    pub(crate) fn parse(path: &'a str) -> Self {
        let bytes = path.as_bytes();
        match bytes {
            [first, ..] if win32::is_separator(char::from(*first)) => {
                Self::unc(path).unwrap_or(Self::Rooted)
            }
            [letter, b':', rest @ ..] if letter.is_ascii_alphabetic() => Self::Drive {
                prefix: &path[..2],
                absolute: rest
                    .first()
                    .is_some_and(|byte| win32::is_separator(char::from(*byte))),
            },
            _ => Self::Relative,
        }
    }

    /// Match `\\server\share`, with non-empty components and any run of separators between.
    fn unc(path: &'a str) -> Option<Self> {
        let after_slashes = path
            .strip_prefix(win32::is_separator)?
            .strip_prefix(win32::is_separator)?;
        let server_len = after_slashes
            .find(win32::is_separator)
            .filter(|len| *len > 0)?;
        let (server, after_server) = after_slashes.split_at(server_len);
        let share_start = after_server.find(|c| !win32::is_separator(c))?;
        let after_gap = &after_server[share_start..];
        let share_len = after_gap
            .find(win32::is_separator)
            .unwrap_or(after_gap.len());
        Some(Self::Unc {
            server,
            share: &after_gap[..share_len],
            end: 2 + server_len + share_start + share_len,
        })
    }
}

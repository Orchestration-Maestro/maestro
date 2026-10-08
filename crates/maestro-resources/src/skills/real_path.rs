//! Real-path resolution that follows the JavaScript runtime's link walk.

use super::operations::{ResourceOperations, with_process_context};
use maestro_path::{Cwd, SEP, dirname, resolve};
use std::{
    borrow::Cow,
    io,
    path::{Path, PathBuf},
};

/// Link expansions one resolution may need on Linux, its `MAXSYMLINKS` (see `path_resolution(7)`).
#[cfg(target_os = "linux")]
const MAX_LINK_EXPANSIONS: usize = 40;
/// Reparse points one resolution may need on Windows.
#[cfg(windows)]
const MAX_LINK_EXPANSIONS: usize = 63;
/// Link expansions one resolution may need on macOS and the BSDs: their
/// `MAXSYMLINKS` in `sys/param.h`.
#[cfg(not(any(target_os = "linux", windows)))]
const MAX_LINK_EXPANSIONS: usize = 32;

/// Resolve `path` against the process working directory, then follow its links.
///
/// `.` and `..` fold lexically before any component is inspected, so `link/..`
/// names the directory that holds `link`. The first link component is then
/// replaced by its target, read relative to the link's directory, and the walk
/// restarts from the root until no component is a link. Components that are not
/// links keep their authored spelling and case, and a Windows link target that
/// names a drive or share loses its verbatim prefix. On Windows a drive-relative
/// path continues from the drive directory `operations` reports. The walk ends
/// like the platform's own path resolution: it fails once it needs more link
/// expansions than the platform allows, so links that fold back into each other
/// do not loop forever. A relative path is inspected from `.`, so an unreadable
/// working directory fails it.
///
/// # Errors
/// Returns the cause of the first failed observation: a path that is not UTF-8,
/// a component that cannot be inspected, a link whose target is missing or loops,
/// a walk that needs more link expansions than the platform allows, or a link
/// that cannot be read.
pub(super) fn real_path(path: &Path, operations: &dyn ResourceOperations) -> io::Result<PathBuf> {
    let path = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    with_process_context(operations, |ctx| follow_links(path, ctx))
}

/// Fold `path` lexically, then expand its links until none remains.
fn follow_links(path: &str, ctx: &Cwd<'_>) -> io::Result<PathBuf> {
    let mut resolved = resolve(&[path], ctx);
    for _ in 0..=MAX_LINK_EXPANSIONS {
        match expand_first_link(&resolved, ctx)? {
            Some(expanded) => resolved = expanded,
            None => return Ok(PathBuf::from(resolved)),
        }
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

/// Replace the first link component of a resolved path by its target.
///
/// Returns `None` when no component is a link.
fn expand_first_link(path: &str, ctx: &Cwd<'_>) -> io::Result<Option<String>> {
    let root = root_len(path);
    std::fs::symlink_metadata(&path[..root])?;
    let mut parent_end = root;
    let ends = path[root..]
        .match_indices(SEP)
        .map(|(at, _)| root + at)
        .chain(std::iter::once(path.len()));
    for end in ends {
        let link = &path[..end];
        if std::fs::symlink_metadata(link)?.is_symlink() {
            std::fs::metadata(link)?;
            let target = std::fs::read_link(link)?.to_string_lossy().into_owned();
            let target = if cfg!(windows) {
                plain_link_target(&target)
            } else {
                Cow::Borrowed(target.as_str())
            };
            let linked = resolve(&[&path[..parent_end], &target], ctx);
            let rest = path[end..].trim_start_matches(SEP);
            return Ok(Some(resolve(&[&linked, rest], ctx)));
        }
        parent_end = end;
    }
    Ok(None)
}

/// Byte length of a resolved path's root: the fixed point of `dirname`.
fn root_len(path: &str) -> usize {
    let mut root = path.to_owned();
    loop {
        let parent = dirname(&root);
        if parent == root {
            return root.len();
        }
        root = parent;
    }
}

/// Drop the verbatim prefix from a drive or share target, as the runtime's link reader does.
///
/// Any other target, including a verbatim volume or a drive colon followed by
/// other text, is returned unchanged.
fn plain_link_target(target: &str) -> Cow<'_, str> {
    let Some(rest) = target.strip_prefix(r"\\?\") else {
        return Cow::Borrowed(target);
    };
    let is_drive = matches!(
        rest.as_bytes(),
        [letter, b':'] | [letter, b':', b'\\', ..] if letter.is_ascii_alphabetic()
    );
    if is_drive {
        return Cow::Borrowed(rest);
    }
    match rest.split_at_checked(4) {
        Some((unc, share)) if unc.eq_ignore_ascii_case(r"UNC\") => {
            Cow::Owned(format!(r"\\{share}"))
        }
        _ => Cow::Borrowed(target),
    }
}

#[cfg(test)]
mod tests {
    use super::plain_link_target;

    /// Drive and share roots lose the verbatim prefix; every other target is kept.
    #[test]
    fn verbatim_link_targets_return_the_plain_form() {
        for (reported, plain) in [
            (r"\\?\C:\Users\skills", r"C:\Users\skills"),
            (r"\\?\c:", "c:"),
            (r"\\?\UNC\server\share\skills", r"\\server\share\skills"),
            (r"\\?\unc\server\share", r"\\server\share"),
            (r"\\?\C:skills", r"\\?\C:skills"),
            (r"\\?\Volume{1}\skills", r"\\?\Volume{1}\skills"),
            (r"\\?\UNCserver", r"\\?\UNCserver"),
            (r"C:\Users\skills", r"C:\Users\skills"),
            (r"\\server\share", r"\\server\share"),
            (r"..\skills", r"..\skills"),
            ("", ""),
        ] {
            assert_eq!(plain_link_target(reported), plain, "{reported}");
        }
    }
}

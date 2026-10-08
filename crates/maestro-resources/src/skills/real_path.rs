//! Real-path resolution that follows the JavaScript runtime's link walk.

use maestro_path::{Cwd, SEP, dirname, is_absolute, resolve};
use std::{
    borrow::Cow,
    io,
    path::{Path, PathBuf},
};

/// Resolve `path` against the process working directory, then follow its links.
///
/// `.` and `..` fold lexically before any component is inspected, so `link/..`
/// names the directory that holds `link`. The first link component is then
/// replaced by its target, read relative to the link's directory, and the walk
/// restarts from the root until no component is a link. Components that are not
/// links keep their authored spelling and case, and a Windows link target that
/// names a drive or share loses its verbatim prefix. An unreadable working
/// directory fails only a relative path.
///
/// # Errors
/// Returns the cause of the first failed observation: a path that is not UTF-8,
/// a relative path with an unreadable working directory, a component that cannot
/// be inspected, a link whose target is missing or loops, or a link that cannot
/// be read.
pub(super) fn real_path(path: &Path) -> io::Result<PathBuf> {
    let path = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    let current = std::env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .or_else(|error| {
            if is_absolute(path) {
                Ok(String::new())
            } else {
                Err(error)
            }
        })?;
    let cwd = Cwd {
        current: &current,
        drive_directories: &[],
    };
    let mut resolved = resolve(&[path], &cwd);
    while let Some(expanded) = expand_first_link(&resolved, &cwd)? {
        resolved = expanded;
    }
    Ok(PathBuf::from(resolved))
}

/// Replace the first link component of a resolved path by its target.
///
/// Returns `None` when no component is a link.
fn expand_first_link(path: &str, cwd: &Cwd<'_>) -> io::Result<Option<String>> {
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
            let linked = resolve(&[&path[..parent_end], &target], cwd);
            let rest = path[end..].trim_start_matches(SEP);
            return Ok(Some(resolve(&[&linked, rest], cwd)));
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

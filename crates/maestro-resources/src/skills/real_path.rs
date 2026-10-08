//! Real-path resolution that follows the JavaScript runtime's link walk.

use super::operations::{ResourceOperations, with_process_context};
use maestro_path::{Cwd, SEP, dirname, is_absolute, resolve};
use std::path::{Path, PathBuf};
use std::{borrow::Cow, collections::HashMap, io};

/// Resolve `path` against the process working directory, then follow its links.
///
/// `.` and `..` fold lexically before any component is inspected, so `link/..`
/// names the directory that holds `link`. The first link component is then
/// replaced by its target, read relative to the link's directory, and the walk
/// restarts from the root until no component is a link. Components that are not
/// links keep their authored spelling and case, and a Windows link target that
/// names a drive or share loses its verbatim prefix. On Windows a drive-relative
/// path continues from the drive directory `operations` reports. A link may be
/// expanded any number of times, as in `loop/loop/x` with `loop` linked to its
/// own directory, except that expanding it again while everything that followed
/// it at its previous expansion still ends the path after it cannot progress, so
/// the walk fails instead of looping forever. A path that is still relative after
/// the lexical resolution needs the working directory, so it fails when
/// `operations` cannot read it; an absolute path does not.
///
/// # Errors
/// Returns the first failure: a path that is not UTF-8, a relative path whose
/// working directory cannot be read (the kind of that error), a component that
/// cannot be inspected, a link whose target is missing or loops, a link whose
/// expansion cannot progress, or a link that cannot be read.
pub(super) fn real_path(path: &Path, operations: &dyn ResourceOperations) -> io::Result<PathBuf> {
    let path = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    with_process_context(operations, |ctx, unreadable| {
        let mut resolved = resolve(&[path], ctx);
        if let Some(kind) = unreadable.filter(|_| !is_absolute(&resolved)) {
            return Err(kind.into());
        }
        let mut seen = Seen::new();
        while let Some(next) = expand_first_link(&resolved, ctx, &mut seen)? {
            resolved = next;
        }
        Ok(PathBuf::from(resolved))
    })
}

/// The components after each link location when it was last expanded.
type Seen = HashMap<String, String>;

/// Replace the first link component of an absolute resolved path by its target.
///
/// Returns `None` when no component is a link. Fails when that link was expanded
/// before and everything that followed it then still ends the path after it.
fn expand_first_link(path: &str, ctx: &Cwd<'_>, seen: &mut Seen) -> io::Result<Option<String>> {
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
            let rest = path[end..].trim_start_matches(SEP);
            let before = seen.insert(link.to_owned(), rest.to_owned());
            if before.is_some_and(|before| Path::new(rest).ends_with(before)) {
                return Err(io::Error::other("symbolic link expansion cannot progress"));
            }
            let target = std::fs::read_link(link)?.to_string_lossy().into_owned();
            let target = if cfg!(windows) {
                plain_link_target(&target)
            } else {
                Cow::Borrowed(target.as_str())
            };
            let linked = resolve(&[&path[..parent_end], &target], ctx);
            return Ok(Some(resolve(&[&linked, rest], ctx)));
        }
        parent_end = end;
    }
    Ok(None)
}

/// Byte length of an absolute resolved path's root: the fixed point of `dirname`,
/// which is a leading part of the path.
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

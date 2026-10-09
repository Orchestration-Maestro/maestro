//! Real-path resolution that follows the JavaScript runtime's link walk.

use super::operations::{ProcessContext, ResourceOperations};
use maestro_path::{SEP, dirname};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{borrow::Cow, io};

/// Resolve `path` against the process working directory, then follow its links.
///
/// `.` and `..` fold lexically before any component is inspected, so `link/..`
/// names the directory that holds `link`. The scan then walks the components
/// from the root. At the first link it reads the target relative to the link's
/// directory, appends the components not yet scanned, and restarts from the
/// root, until no component is a link. Components that are not links keep their authored spelling and case,
/// and a Windows link target that names a drive or share loses its verbatim
/// prefix. On Windows a drive-relative path continues from the drive directory
/// `operations` reports. A link may be expanded any number of times, as in
/// `loop/loop/x` with `loop` linked to its own directory. The walk fails when a
/// link is expanded again before the scan has read any component of what
/// followed it at its previous expansion, because the steps in between would
/// repeat forever. A path its operands leave unanchored needs the working
/// directory, so the resolution fails with the kind of the error `operations`
/// reports when it cannot read it; an anchored path does not.
///
/// # Errors
/// Returns the first failure: a path that is not UTF-8, a path that needs a
/// working directory `operations` cannot read (that error's kind), a component
/// that cannot be inspected, a link whose target is missing or loops, a link
/// that cannot be read, or a link expanded again before the scan has read any
/// component of what followed its previous expansion.
pub(super) fn real_path(path: &Path, operations: &dyn ResourceOperations) -> io::Result<PathBuf> {
    let path = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    let ctx = ProcessContext::new(operations);
    let mut resolved = ctx.resolve(&[path])?;
    let mut walk = Walk::default();
    while let Some(next) = walk.expand_first_link(&resolved, &ctx)? {
        resolved = next;
    }
    Ok(PathBuf::from(resolved))
}

/// What the walk remembers across restarts.
#[derive(Default)]
struct Walk {
    /// Prefixes found not to be links.
    hard: HashSet<String>,
    /// Each expanded link, with the byte length of the rest that followed it
    /// until the scan reads a component of that rest.
    pending: HashMap<String, usize>,
}

impl Walk {
    /// Replace the first link of an absolute resolved path by its target and
    /// the components after it, or return `None` when no component is a link.
    ///
    /// The path is normalized, so the rest after an expanded link stays
    /// verbatim at the end of the path; the scan has read into it once fewer
    /// bytes than its length remain after the current component.
    fn expand_first_link(
        &mut self,
        path: &str,
        ctx: &ProcessContext<'_>,
    ) -> io::Result<Option<String>> {
        let root = root_len(path);
        if cfg!(windows) {
            std::fs::symlink_metadata(&path[..root])?;
        }
        let mut parent_end = root;
        for end in component_ends(path, root) {
            self.pending.retain(|_, len| *len <= path.len() - end);
            if self.is_link(&path[..end])? {
                return self.expand(path, (parent_end, end), ctx).map(Some);
            }
            parent_end = end;
        }
        Ok(None)
    }

    /// Inspect `prefix` unless it is already known not to be a link.
    fn is_link(&mut self, prefix: &str) -> io::Result<bool> {
        if self.hard.contains(prefix) {
            return Ok(false);
        }
        let link = std::fs::symlink_metadata(prefix)?.is_symlink();
        if !link {
            self.hard.insert(prefix.to_owned());
        }
        Ok(link)
    }

    /// Replace the link `path[..end]`, whose parent ends at `parent_end`, by its
    /// target followed by the components after it.
    ///
    /// Fails when the link was expanded before and the scan has read no
    /// component of what followed it then.
    fn expand(
        &mut self,
        path: &str,
        (parent_end, end): (usize, usize),
        ctx: &ProcessContext<'_>,
    ) -> io::Result<String> {
        let link = &path[..end];
        std::fs::metadata(link)?;
        let rest = &path[end..];
        if self.pending.insert(link.to_owned(), rest.len()).is_some() {
            return Err(io::Error::other(
                "symbolic link expansion never reaches the rest of the path",
            ));
        }
        let target = std::fs::read_link(link)?.to_string_lossy().into_owned();
        let target = if cfg!(windows) {
            plain_link_target(&target)
        } else {
            Cow::Borrowed(target.as_str())
        };
        let linked = ctx.resolve(&[&path[..parent_end], &target])?;
        ctx.resolve(&[&linked, rest.trim_start_matches(SEP)])
    }
}

/// The end offset of each component of `path` after its root of `root` bytes.
fn component_ends(path: &str, root: usize) -> impl Iterator<Item = usize> {
    path[root..]
        .match_indices(SEP)
        .map(move |(at, _)| root + at)
        .chain(std::iter::once(path.len()))
        .filter(move |end| *end > root)
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

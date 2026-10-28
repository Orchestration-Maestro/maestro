//! Mapping a merge reference through a remote's fetch refspecs.

/// The local reference that tracks `merge_ref` on `remote`, when its refspecs map it.
///
/// A remote of `.` is the repository itself. A remote without refspecs uses the default
/// `+refs/heads/*:refs/remotes/<remote>/*`. A negative refspec only filters what a fetch
/// downloads, so it never removes a mapping.
pub(super) fn tracking_ref<'a>(
    remote: &str,
    merge_ref: &str,
    refspecs: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    if remote == "." {
        return Some(merge_ref.to_owned());
    }
    let mut refspecs = refspecs.into_iter().peekable();
    if refspecs.peek().is_none() {
        let default = format!("+refs/heads/*:refs/remotes/{remote}/*");
        return map(&default, merge_ref);
    }
    refspecs.find_map(|refspec| map(refspec, merge_ref))
}

/// Apply one refspec to a reference.
fn map(refspec: &str, merge_ref: &str) -> Option<String> {
    let refspec = refspec.strip_prefix('+').unwrap_or(refspec);
    let (source, destination) = refspec.split_once(':')?;
    if source.starts_with('^') {
        return None;
    }
    match (source.split_once('*'), destination.split_once('*')) {
        (None, None) => (source == merge_ref).then(|| destination.to_owned()),
        (Some((source_prefix, source_suffix)), Some((prefix, suffix))) => {
            let matched = merge_ref
                .strip_prefix(source_prefix)?
                .strip_suffix(source_suffix)?;
            Some(format!("{prefix}{matched}{suffix}"))
        }
        _ => None,
    }
}

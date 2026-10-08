//! Segment reduction shared by both path flavors.

/// Drop empty and `.` segments and cancel each `..` against the segment before it.
///
/// A `..` with no segment left to cancel, or following another kept `..`, is
/// kept when `allow_above_root` is set and dropped otherwise.
pub(super) fn reduce<'a>(
    segments: impl Iterator<Item = &'a str>,
    allow_above_root: bool,
) -> Vec<&'a str> {
    let mut kept: Vec<&str> = Vec::new();
    for segment in segments {
        match segment {
            "" | "." => {}
            ".." => match kept.last() {
                Some(&last) if last != ".." => {
                    kept.pop();
                }
                _ if allow_above_root => kept.push(".."),
                _ => {}
            },
            other => kept.push(other),
        }
    }
    kept
}

/// The last component of `path[start..]`, ignoring trailing separators, without `suffix`.
///
/// The suffix is a literal, case-sensitive ending of that component; it is not
/// removed when it would leave nothing, except that a suffix equal to the whole
/// path yields an empty name. A path of only separators has no component, so
/// the result is empty whatever the suffix.
pub(super) fn base_name<'a>(
    path: &'a str,
    start: usize,
    suffix: Option<&str>,
    is_separator: impl Fn(char) -> bool + Copy,
) -> &'a str {
    let component = path[start..]
        .trim_end_matches(is_separator)
        .rsplit(is_separator)
        .next()
        .unwrap_or_default();
    match suffix {
        Some(suffix) if !suffix.is_empty() && suffix == path => "",
        Some(suffix) if !suffix.is_empty() => component
            .strip_suffix(suffix)
            .filter(|stem| !stem.is_empty())
            .unwrap_or(component),
        _ => component,
    }
}

/// The path that leaves `unshared_from` components with `..` and then descends `to_rest`.
pub(super) fn climb_and_descend(unshared_from: usize, to_rest: &[&str], separator: &str) -> String {
    let mut steps = vec![".."; unshared_from];
    steps.extend_from_slice(to_rest);
    steps.join(separator)
}

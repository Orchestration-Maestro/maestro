//! The source files a dependency file names, as Cargo and the compiler write it beside an output.

/// The paths after the target in the first rule of the dependency file `text`, or `None` when it
/// holds no rule.
///
/// A path with a space is written with a backslash before the space, which splits it into two
/// entries here; the first ends in a backslash.
pub(crate) fn sources(text: &str) -> Option<std::str::SplitWhitespace<'_>> {
    let (_, sources) = text.lines().next()?.split_once(": ")?;
    Some(sources.split_whitespace())
}

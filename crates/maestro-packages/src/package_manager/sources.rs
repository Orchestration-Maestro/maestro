//! Source classification and npm package-name selection.
use crate::{GitSource, parse_git_url};
/// A source's identity-bearing kind.
pub(super) enum Source<'a> {
    /// A parsed npm name.
    Npm(&'a str),
    /// A selected repository.
    Git(GitSource),
    /// An authored local path.
    Local(&'a str),
}
/// Applies literal npm precedence, then shared local admission, then Git.
pub(super) fn parse(source: &str) -> Source<'_> {
    if let Some(spec) = source.strip_prefix("npm:") {
        return Source::Npm(npm_name(crate::trim(spec)));
    }
    if !maestro_resources::is_local_path(source)
        && let Some(git) = parse_git_url(source)
    {
        return Source::Git(git);
    }
    Source::Local(source)
}
/// Selects the name only when an explicit suffix satisfies the source pattern.
fn npm_name(spec: &str) -> &str {
    let offset = usize::from(spec.starts_with('@'));
    if let Some(index) = spec[offset..].find('@').map(|index| index + offset) {
        let suffix = &spec[index + 1..];
        if index > offset
            && !suffix.is_empty()
            && !suffix.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
        {
            return &spec[..index];
        }
    }
    spec
}

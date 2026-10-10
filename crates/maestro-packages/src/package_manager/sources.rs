//! Source classification and npm package-name selection.
use crate::{GitSource, parse_git_url};
/// A source's identity-bearing kind.
pub(super) enum Source<'a> {
    /// An npm package: the full trimmed spec and the name selected from it.
    Npm {
        /// The trimmed text after the `npm:` prefix.
        spec: &'a str,
        /// The package name within the spec.
        name: &'a str,
    },
    /// A selected repository.
    Git(GitSource),
    /// An authored local path.
    Local(&'a str),
}
/// Applies literal npm precedence, then shared local admission, then Git.
pub(super) fn parse(source: &str) -> Source<'_> {
    if let Some(spec) = source.strip_prefix("npm:") {
        let spec = crate::trim(spec);
        return Source::Npm {
            spec,
            name: npm_name(spec),
        };
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

/// Builds the configured lookup failure with the first matching shorthand.
pub(super) fn no_match_message(source: &str, configured: &[String]) -> String {
    let input = crate::trim(source);
    let suggestion = configured.iter().find(|stored| match parse(stored) {
        Source::Npm { name, spec } => input == name || input == spec,
        Source::Git(git) => {
            let shorthand = format!("{}/{}", git.host, git.path);
            input == shorthand
                || git
                    .r#ref
                    .is_some_and(|reference| input == format!("{shorthand}@{reference}"))
        }
        Source::Local(_) => false,
    });
    suggestion.map_or_else(
        || format!("No matching package found for {source}"),
        |suggestion| format!("No matching package found for {source}. Did you mean {suggestion}?"),
    )
}

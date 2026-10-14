//! Package path spellings.
/// Split at the final forward slash, retaining the folder's slash.
pub(super) fn split_path(path: &str) -> (&str, &str) {
    path.split_at(path.rfind('/').map_or(0, |i| i + 1))
}
/// Remove exactly one package-root slash.
pub(super) fn normalize_path(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}
/// Resolve a relationship target against the package URL base.
pub(super) fn resolve_path(path: &str, base: &str) -> String {
    url::Url::parse(&format!("http://docx/{base}"))
        .and_then(|base| base.join(path))
        .map_or_else(
            |_| format!("{base}{path}"),
            |url| url.as_str().chars().skip(12).collect(),
        )
}
/// Admit archive dot segments without decoding escapes or backslashes.
pub(super) fn archive_name(path: &str) -> String {
    let mut result = Vec::new();
    let mut parts = path.split('/').enumerate().peekable();
    while let Some((index, part)) = parts.next() {
        match part {
            "." => {}
            "" if index != 0 && parts.peek().is_some() => {}
            ".." => {
                result.pop();
            }
            _ => result.push(part),
        }
    }
    result.join("/")
}

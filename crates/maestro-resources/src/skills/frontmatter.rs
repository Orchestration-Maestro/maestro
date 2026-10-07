//! Typed metadata and delimited Markdown bodies.

/// Owned metadata returned by the YAML parser.
#[derive(Debug, Clone, PartialEq)]
pub enum FrontmatterValue {
    /// A null scalar.
    Null,
    /// A boolean scalar.
    Bool(bool),
    /// A numeric scalar.
    Number(f64),
    /// A text scalar.
    String(String),
    /// An ordered sequence.
    Sequence(Vec<FrontmatterValue>),
    /// An insertion-ordered mapping with typed keys.
    Mapping(Vec<(FrontmatterValue, FrontmatterValue)>),
}
/// Metadata read from a skill file.
pub type SkillFrontmatter = FrontmatterValue;
/// Parsed metadata with its Markdown body.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFrontmatter {
    /// Metadata, or an empty mapping when absent or null.
    pub frontmatter: FrontmatterValue,
    /// Normalized body, trimmed only after a complete header.
    pub body: String,
}
/// A resource operation failure.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceError {
    /// Original error text, absent for a non-error failure.
    pub message: Option<String>,
}
/// Parses metadata, preserving normalized text when no complete delimiter exists.
pub fn parse_frontmatter(content: &str) -> Result<ParsedFrontmatter, ResourceError> {
    let (yaml, body) = extract_frontmatter(content);
    let frontmatter = match yaml {
        Some(yaml) if !yaml.is_empty() => match parse_yaml(&yaml)? {
            FrontmatterValue::Null => FrontmatterValue::Mapping(vec![]),
            value => value,
        },
        _ => FrontmatterValue::Mapping(vec![]),
    };
    Ok(ParsedFrontmatter { frontmatter, body })
}
fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}
fn extract_frontmatter(content: &str) -> (Option<String>, String) {
    let normalized = normalize_newlines(content);
    if !normalized.starts_with("---") {
        return (None, normalized);
    }
    let Some(end) = normalized[3..].find("\n---").map(|i| i + 3) else {
        return (None, normalized);
    };
    let yaml = normalized[3..end].chars().skip(1).collect();
    let body = normalized[end + 4..].trim().to_owned();
    (Some(yaml), body)
}
/// Returns the parsed body; malformed metadata errors are propagated.
pub fn strip_frontmatter(content: &str) -> Result<String, ResourceError> {
    Ok(parse_frontmatter(content)?.body)
}
fn parse_yaml(text: &str) -> Result<FrontmatterValue, ResourceError> {
    let docs = yaml_rust2::YamlLoader::load_from_str(text).map_err(|e| ResourceError {
        message: Some(e.to_string()),
    })?;
    docs.first()
        .map(convert)
        .unwrap_or(Ok(FrontmatterValue::Null))
}
fn convert(value: &yaml_rust2::Yaml) -> Result<FrontmatterValue, ResourceError> {
    use FrontmatterValue as V;
    use yaml_rust2::Yaml;
    Ok(match value {
        Yaml::Null | Yaml::BadValue | Yaml::Alias(_) => V::Null,
        Yaml::Boolean(b) => V::Bool(*b),
        Yaml::Integer(n) => V::Number(*n as f64),
        Yaml::Real(_) => V::Number(value.as_f64().unwrap_or(f64::NAN)),
        Yaml::String(s) => V::String(s.clone()),
        Yaml::Array(a) => V::Sequence(a.iter().map(convert).collect::<Result<_, _>>()?),
        Yaml::Hash(m) => V::Mapping(
            m.iter()
                .map(|(k, v)| Ok((convert(k)?, convert(v)?)))
                .collect::<Result<_, ResourceError>>()?,
        ),
    })
}

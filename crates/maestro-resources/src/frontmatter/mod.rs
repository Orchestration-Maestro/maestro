//! Extract authored metadata without trimming ordinary text.

mod alias_count;
mod composer;
mod cst;
mod lexer;
mod parser;
mod resolve_block_map;
mod resolve_block_scalar;
mod resolve_block_seq;
mod resolve_flow_collection;
mod resolve_flow_scalar;
mod resolve_props;
mod values;

/// An ordered YAML value retaining scalar kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum FrontmatterValue {
    /// A null scalar.
    Null,
    /// A boolean scalar.
    Bool(bool),
    /// A numeric scalar, including nonfinite values.
    Number(f64),
    /// A text scalar.
    String(String),
    /// An ordered collection.
    Sequence(Vec<Self>),
    /// An insertion-ordered string-keyed collection.
    Mapping(Vec<(String, Self)>),
}

/// Metadata with its extracted document body.
#[derive(Debug, PartialEq)]
pub struct ParsedFrontmatter {
    /// Resolved metadata.
    pub frontmatter: FrontmatterValue,
    /// Body text, trimmed only when a header is present.
    pub body: String,
}

/// A metadata failure with source coordinates when available.
#[derive(Debug, PartialEq)]
pub struct FrontmatterError {
    /// Useful parser or field-validation cause.
    pub message: String,
    /// One-based source line.
    pub line: Option<usize>,
    /// One-based source column.
    pub column: Option<usize>,
}

impl std::fmt::Display for FrontmatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for FrontmatterError {}

impl FrontmatterError {
    /// Construct a validation error without source coordinates.
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
            column: None,
        }
    }
}

/// Parse an optional leading metadata header.
///
/// Double-quoted metadata strings accept adjacent four-digit high/low surrogate
/// escape pairs as one Unicode scalar.
///
/// # Errors
/// Returns a parsing cause when the extracted metadata is invalid.
pub fn parse_frontmatter(content: &str) -> Result<ParsedFrontmatter, FrontmatterError> {
    let normalized = maestro_text::normalize_to_lf(content);
    let Some(end) = normalized
        .strip_prefix("---")
        .and_then(|s| s.find("\n---").map(|n| n + 3))
    else {
        return Ok(ParsedFrontmatter {
            frontmatter: FrontmatterValue::Mapping(Vec::new()),
            body: normalized,
        });
    };
    let start = 3 + normalized[3..].chars().next().map_or(0, char::len_utf8);
    let yaml = normalized.get(start..end).unwrap_or_default();
    let frontmatter = match parse_yaml(yaml)? {
        FrontmatterValue::Null => FrontmatterValue::Mapping(Vec::new()),
        value => value,
    };
    Ok(ParsedFrontmatter {
        frontmatter,
        body: normalized[end + 4..].trim_matches(text_whitespace).into(),
    })
}

/// Return the body after parsing optional metadata.
///
/// # Errors
/// Propagates invalid extracted metadata rather than discarding the cause.
pub fn strip_frontmatter(content: &str) -> Result<String, FrontmatterError> {
    Ok(parse_frontmatter(content)?.body)
}

pub(crate) use maestro_text::is_whitespace_scalar as text_whitespace;

/// Parse one YAML document without extracting a metadata header.
///
/// # Errors
/// Returns the native parser's cause and source coordinates.
pub fn parse_yaml(content: &str) -> Result<FrontmatterValue, FrontmatterError> {
    composer::compose(&maestro_text::normalize_to_lf(content))
}

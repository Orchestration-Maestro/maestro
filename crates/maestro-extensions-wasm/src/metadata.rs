//! Complete native metadata values without a JSON-number boundary.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::bindings::maestro::extension::host::FrontmatterToken;
use crate::{ExtensionAPI, ExtensionResult};
pub use maestro_interchange::metadata::{FrontmatterValue, ParsedFrontmatter};
/// Parses metadata through the installed native owner.
///
/// # Errors
/// Returns the native parser's cause and coordinates, or invalid tree framing.
pub fn parse_frontmatter(api: &ExtensionAPI, content: &str) -> ExtensionResult<ParsedFrontmatter> {
    let document = api.native_frontmatter(content)?;
    let frontmatter = read_document(document.values, true)?;
    Ok(ParsedFrontmatter {
        frontmatter,
        body: document.body,
    })
}
/// Checks that the typed framing contains exactly one complete value.
fn read_document(
    values: Vec<FrontmatterToken>,
    canonical: bool,
) -> ExtensionResult<FrontmatterValue> {
    let mut values = values.into_iter();
    let value = read_value(&mut values, canonical)?;
    if values.next().is_some() {
        return Err("Native metadata contains trailing values".into());
    }
    Ok(value)
}
/// Reconstructs only the typed value framing; YAML parsing stays in the host.
fn read_value(
    values: &mut impl Iterator<Item = FrontmatterToken>,
    canonical: bool,
) -> ExtensionResult<FrontmatterValue> {
    use FrontmatterValue as V;
    Ok(
        match values.next().ok_or("Native metadata is incomplete")? {
            FrontmatterToken::Null => V::Null,
            FrontmatterToken::Boolean(value) => V::Bool(value),
            FrontmatterToken::Number(value) => V::Number(value),
            FrontmatterToken::Text(value) => V::String(value),
            FrontmatterToken::Sequence(count) => V::Sequence(
                (0..count)
                    .map(|_| read_value(values, canonical))
                    .collect::<ExtensionResult<_>>()?,
            ),
            FrontmatterToken::Mapping(keys) => {
                let entries = keys
                    .into_iter()
                    .map(|key| Ok((key, read_value(values, canonical)?)))
                    .collect::<ExtensionResult<Vec<_>>>()?;
                V::Mapping(if canonical {
                    maestro_json::canonical_entries(entries)
                } else {
                    entries
                })
            }
        },
    )
}

/// Returns the body only after the native owner has parsed any header.
///
/// # Errors
/// Preserves malformed metadata causes and coordinates instead of silently stripping it.
pub fn strip_frontmatter(api: &ExtensionAPI, content: &str) -> ExtensionResult<String> {
    api.helper("strip_frontmatter", &(content,))
}

/// Parses one YAML document, retaining null roots and authored mapping order.
///
/// Numbers retain signed zero, NaN and infinities through the typed host boundary.
///
/// # Errors
/// Returns native parsing failures, including source coordinates.
pub fn parse_yaml(api: &ExtensionAPI, content: &str) -> ExtensionResult<FrontmatterValue> {
    read_document(api.native_yaml(content)?, false)
}

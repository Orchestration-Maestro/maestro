//! Skill-field validation preserving authored warning order.

/// Validate a name against its containing directory and allowed spelling.
pub(super) fn name_warnings(name: &str, parent: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if name != parent {
        warnings.push(format!(
            "name \"{name}\" does not match parent directory \"{parent}\""
        ));
    }
    let count = name.chars().count();
    if count > 64 {
        warnings.push(format!("name exceeds 64 characters ({count})"));
    }
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        warnings.push(
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)".into(),
        );
    }
    if name.starts_with('-') || name.ends_with('-') {
        warnings.push("name must not start or end with a hyphen".into());
    }
    if name.contains("--") {
        warnings.push("name must not contain consecutive hyphens".into());
    }
    warnings
}

use crate::{FrontmatterError, FrontmatterValue};

/// Checked supported skill fields and unrelated metadata.
#[derive(Debug, Default)]
pub struct SkillFrontmatter {
    /// Optional declared name.
    pub name: Option<String>,
    /// Optional authored description.
    pub description: Option<String>,
    /// Only explicit boolean true hides a prompt member.
    pub disable_model_invocation: bool,
    /// Unrelated mapping members in insertion order.
    pub extra: Vec<(String, FrontmatterValue)>,
}
impl TryFrom<&FrontmatterValue> for SkillFrontmatter {
    type Error = FrontmatterError;
    fn try_from(value: &FrontmatterValue) -> Result<Self, Self::Error> {
        let mut fields = project_fields(value)?;
        if let FrontmatterValue::Mapping(mapping) = value {
            fields.extra = mapping
                .iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "name" | "description" | "disable-model-invocation"
                    )
                })
                .cloned()
                .collect();
        }
        Ok(fields)
    }
}
/// Project only consumed skill fields, leaving unrelated parser data un-copied.
pub(super) fn project_fields(
    value: &FrontmatterValue,
) -> Result<SkillFrontmatter, FrontmatterError> {
    let mut fields = SkillFrontmatter::default();
    let FrontmatterValue::Mapping(mapping) = value else {
        return Ok(fields);
    };
    if let Some((key, value)) = mapping.iter().find(|(key, _)| key == "description") {
        fields.description = string_field(key, value)?;
    }
    for (key, value) in mapping {
        match key.as_str() {
            "name" => fields.name = string_field(key, value)?,
            "disable-model-invocation" => {
                fields.disable_model_invocation = matches!(value, FrontmatterValue::Bool(true));
            }
            _ => {}
        }
    }
    Ok(fields)
}
/// Read one optional string field without foreign runtime exceptions.
fn string_field(key: &str, value: &FrontmatterValue) -> Result<Option<String>, FrontmatterError> {
    match value {
        FrontmatterValue::Null => Ok(None),
        FrontmatterValue::String(value) => Ok(Some(value.clone())),
        _ => Err(FrontmatterError::new(format!("{key} must be a string"))),
    }
}
/// Description warnings and omission use the same whitespace boundary.
pub(super) fn description_warnings(description: &str) -> Vec<String> {
    if description
        .trim_matches(crate::frontmatter::text_whitespace)
        .is_empty()
    {
        vec!["description is required".into()]
    } else {
        let count = description.chars().count();
        if count > 1024 {
            vec![format!("description exceeds 1024 characters ({count})")]
        } else {
            Vec::new()
        }
    }
}

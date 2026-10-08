//! Stored list members and package sources.

use serde_json::{Map, Value};

use super::records::{Member, object_view};

/// A member of a stored string list; non-string members are kept untouched.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsListEntry {
    /// A string member.
    String(String),
    /// A member of another JSON type, retained as written.
    Unknown(Value),
}

impl SettingsListEntry {
    /// Reads a stored list member.
    fn from_value(value: &Value) -> Self {
        value.as_str().map_or_else(
            || Self::Unknown(value.clone()),
            |text| Self::String(text.to_owned()),
        )
    }

    /// Writes the member as JSON.
    fn into_value(self) -> Value {
        match self {
            Self::String(text) => Value::String(text),
            Self::Unknown(value) => value,
        }
    }
}

impl Member for Vec<SettingsListEntry> {
    fn read(value: &Value) -> Option<Self> {
        let items = value.as_array()?;
        Some(items.iter().map(SettingsListEntry::from_value).collect())
    }

    fn write(self) -> Value {
        Value::Array(
            self.into_iter()
                .map(SettingsListEntry::into_value)
                .collect(),
        )
    }
}

object_view! {
    /// A package source given as an object: its `source` and the resource lists
    /// that select what to load.
    FilteredPackage {
        /// The package source.
        source / set_source: String = "source",
        /// Extension paths to load.
        extensions / set_extensions: Vec<SettingsListEntry> = "extensions",
        /// Skill paths to load.
        skills / set_skills: Vec<SettingsListEntry> = "skills",
        /// Prompt template paths to load.
        prompts / set_prompts: Vec<SettingsListEntry> = "prompts",
        /// Theme paths to load.
        themes / set_themes: Vec<SettingsListEntry> = "themes",
    }
}

/// Whether an object has a string `source` and only arrays as resource lists.
fn is_filtered(map: &Map<String, Value>) -> bool {
    map.get("source").is_some_and(Value::is_string)
        && ["extensions", "skills", "prompts", "themes"]
            .into_iter()
            .all(|key| map.get(key).is_none_or(Value::is_array))
}

/// A configured package source.
#[derive(Clone, Debug, PartialEq)]
pub enum PackageSource {
    /// Load every resource from the named source.
    Source(String),
    /// Load only the resources that the filters select.
    Filtered(FilteredPackage),
    /// An entry whose shape cannot be typed, retained as written.
    Unknown(Value),
}

impl PackageSource {
    /// Reads a stored package entry. An object is typed only when its `source`
    /// is a string and every present resource list is an array.
    pub(crate) fn from_value(value: &Value) -> Self {
        match value {
            Value::String(source) => Self::Source(source.clone()),
            Value::Object(map) if is_filtered(map) => Self::Filtered(FilteredPackage(map.clone())),
            other => Self::Unknown(other.clone()),
        }
    }

    /// Writes the entry as JSON.
    pub(crate) fn into_value(self) -> Value {
        match self {
            Self::Source(source) => Value::String(source),
            Self::Filtered(package) => Value::Object(package.0),
            Self::Unknown(value) => value,
        }
    }
}

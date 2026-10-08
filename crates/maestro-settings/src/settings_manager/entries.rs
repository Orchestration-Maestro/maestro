//! Stored list members and package sources.

use serde_json::{Map, Value};

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
    pub(crate) fn from_value(value: &Value) -> Self {
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

/// Reads a stored list, or `None` when the value is not an array.
pub(crate) fn read_entries(value: &Value) -> Option<Vec<SettingsListEntry>> {
    value
        .as_array()
        .map(|items| items.iter().map(SettingsListEntry::from_value).collect())
}

/// Writes list members as a JSON array.
pub(crate) fn write_entries(entries: Vec<SettingsListEntry>) -> Value {
    Value::Array(
        entries
            .into_iter()
            .map(SettingsListEntry::into_value)
            .collect(),
    )
}

/// Resource filters of a package source; `None` means the filter is absent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PackageFilters {
    /// Extension paths to load.
    pub extensions: Option<Vec<SettingsListEntry>>,
    /// Skill paths to load.
    pub skills: Option<Vec<SettingsListEntry>>,
    /// Prompt template paths to load.
    pub prompts: Option<Vec<SettingsListEntry>>,
    /// Theme paths to load.
    pub themes: Option<Vec<SettingsListEntry>>,
}

/// The stored resource-filter keys of a package source.
const FILTER_KEYS: [&str; 4] = ["extensions", "skills", "prompts", "themes"];

/// A configured package source.
#[derive(Clone, Debug, PartialEq)]
pub enum PackageSource {
    /// Load every resource from the named source.
    Source(String),
    /// Load only the resources that the filters select.
    Filtered {
        /// The package source.
        source: String,
        /// Which resources to load.
        filters: PackageFilters,
        /// Other members of the stored object.
        extra: Map<String, Value>,
    },
    /// An entry whose shape cannot be typed, retained as written.
    Unknown(Value),
}

impl PackageSource {
    /// Reads a stored package entry.
    pub(crate) fn from_value(value: &Value) -> Self {
        match value {
            Value::String(source) => Self::Source(source.clone()),
            Value::Object(map) => {
                Self::filtered(map).unwrap_or_else(|| Self::Unknown(value.clone()))
            }
            other => Self::Unknown(other.clone()),
        }
    }

    /// Types an object entry, or `None` when its source or a filter is malformed.
    fn filtered(map: &Map<String, Value>) -> Option<Self> {
        let source = map.get("source")?.as_str()?.to_owned();
        let mut lists: [Option<Vec<SettingsListEntry>>; 4] = Default::default();
        for (slot, key) in lists.iter_mut().zip(FILTER_KEYS) {
            if let Some(value) = map.get(key) {
                *slot = Some(read_entries(value)?);
            }
        }
        let [extensions, skills, prompts, themes] = lists;
        let extra = map
            .iter()
            .filter(|(key, _)| key.as_str() != "source" && !FILTER_KEYS.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Some(Self::Filtered {
            source,
            filters: PackageFilters {
                extensions,
                skills,
                prompts,
                themes,
            },
            extra,
        })
    }

    /// Writes the entry as JSON.
    pub(crate) fn into_value(self) -> Value {
        match self {
            Self::Source(source) => Value::String(source),
            Self::Unknown(value) => value,
            Self::Filtered {
                source,
                filters,
                extra,
            } => {
                let mut map = Map::new();
                map.insert("source".to_owned(), Value::String(source));
                map.extend(filters.into_members());
                for (key, value) in extra {
                    map.entry(key).or_insert(value);
                }
                Value::Object(map)
            }
        }
    }
}

impl PackageFilters {
    /// The filters that are present, as stored members.
    fn into_members(self) -> impl Iterator<Item = (String, Value)> {
        let lists = [self.extensions, self.skills, self.prompts, self.themes];
        FILTER_KEYS
            .into_iter()
            .zip(lists)
            .filter_map(|(key, list)| list.map(|entries| (key.to_owned(), write_entries(entries))))
    }
}

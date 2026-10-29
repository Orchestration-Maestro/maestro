//! The areas of a release's manifest root, read in order and combined.

use std::collections::BTreeSet;

use maestro_json::merge_one_level;
use serde_json::{Map, Value};

/// Settings keys that name resource folders; the areas' folders supply those resources.
const RESOURCE_KEYS: [&str; 4] = ["extensions", "skills", "prompts", "themes"];

/// Keys that only the common, standards and core areas may set or lock.
const SELECTION_KEYS: [&str; 4] = [
    "manifestAreas",
    "manifest",
    "minimumRelease",
    "updateCheckMinutes",
];

/// Policy members that only the common, standards and core areas may hold.
const SELECTION_MEMBERS: [&str; 2] = ["minimumRelease", "updateCheckMinutes"];

/// Read access to the files under a release's manifest root.
pub trait ReleaseFiles {
    /// Names of the folders directly under a root-relative folder, sorted; empty when it is absent.
    fn folders(&self, folder: &str) -> Vec<String>;

    /// Text of a root-relative file, or `None` when it is absent.
    ///
    /// # Errors
    /// Returns the reason an existing file could not be read.
    fn text(&self, path: &str) -> Result<Option<String>, String>;
}

/// The settings and locks of every area a manifest loads.
#[derive(Debug, Default, PartialEq)]
pub struct Composition {
    /// The areas' settings documents, combined in area order.
    pub settings: Map<String, Value>,
    /// The key paths and lock names the areas' policies lock.
    pub locked: BTreeSet<String>,
}

/// Reads the common, standards and core areas, then the listed languages and capabilities.
///
/// `requested` is the user's or project's `manifestAreas` value; without one the manifest's own
/// value applies.
///
/// # Errors
/// Returns the problem with the first area file or entry that makes the manifest invalid.
pub fn read(files: &impl ReleaseFiles, requested: Option<&Value>) -> Result<Composition, String> {
    let mut composition = Composition::default();
    let standards = files
        .folders("standards")
        .into_iter()
        .map(|domain| format!("standards/{domain}"));
    let always = std::iter::once(String::new())
        .chain(standards)
        .chain(std::iter::once("core".to_owned()));
    for area in always {
        absorb(&mut composition, files, &area, false)?;
    }
    let listed = requested
        .cloned()
        .or_else(|| composition.settings.get("manifestAreas").cloned());
    for area in listed_areas(files, listed.as_ref())? {
        absorb(&mut composition, files, &area, true)?;
    }
    Ok(composition)
}

/// Joins an area folder (empty for the common area) and a path below it.
fn join(area: &str, path: &str) -> String {
    if area.is_empty() {
        path.to_owned()
    } else {
        format!("{area}/{path}")
    }
}

/// Adds one area's settings and locks; `narrow` areas may not set the selection members.
fn absorb(
    composition: &mut Composition,
    files: &impl ReleaseFiles,
    area: &str,
    narrow: bool,
) -> Result<(), String> {
    let settings_path = join(area, "settings/settings.json");
    let policy_path = join(area, "settings/policy.json");
    let settings = document(files, &settings_path)?;
    let policy = document(files, &policy_path)?;
    forbid(&RESOURCE_KEYS, &settings_path, |key| {
        settings.contains_key(key)
    })?;
    let locked = locks(&policy, &policy_path)?;
    if narrow {
        forbid(&SELECTION_KEYS, &settings_path, |key| {
            settings.contains_key(key)
        })?;
        forbid(&SELECTION_MEMBERS, &policy_path, |member| {
            policy.contains_key(member)
        })?;
        forbid(&SELECTION_KEYS, &policy_path, |key| locked.contains(key))?;
    }
    composition.settings = merge_one_level(std::mem::take(&mut composition.settings), settings);
    composition.locked.extend(locked);
    Ok(())
}

/// Refuses the first of `names` that `present` finds in the file at `path`.
fn forbid(names: &[&str], path: &str, present: impl Fn(&str) -> bool) -> Result<(), String> {
    match names.iter().find(|name| present(name)) {
        Some(name) => Err(format!("{path}: `{name}` is not allowed here")),
        None => Ok(()),
    }
}

/// Reads a JSON object from a root-relative file; an absent file is an empty object.
fn document(files: &impl ReleaseFiles, path: &str) -> Result<Map<String, Value>, String> {
    let Some(text) = files.text(path)? else {
        return Ok(Map::new());
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(format!("{path}: must be a JSON object")),
        Err(error) => Err(format!("{path}: not valid JSON ({error})")),
    }
}

/// Reads a policy's `locked` list of key paths and lock names.
fn locks(policy: &Map<String, Value>, path: &str) -> Result<BTreeSet<String>, String> {
    let Some(locked) = policy.get("locked") else {
        return Ok(BTreeSet::new());
    };
    locked
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .ok_or_else(|| format!("{path}: `locked` must be a list of key paths"))
}

/// The listed language areas, then the listed capability areas, each in list order, once.
fn listed_areas(files: &impl ReleaseFiles, listed: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(listed) = listed else {
        return Ok(Vec::new());
    };
    let not_a_list = || "`manifestAreas` must be a list of area paths".to_owned();
    let entries = listed.as_array().ok_or_else(not_a_list)?;
    let mut languages = Vec::new();
    let mut capabilities = Vec::new();
    for entry in entries {
        let entry = entry.as_str().ok_or_else(not_a_list)?;
        let kind = match entry.split('/').collect::<Vec<_>>().as_slice() {
            ["languages", name] if is_name(name) => &mut languages,
            ["capabilities", group, name] if is_name(group) && is_name(name) => &mut capabilities,
            _ => {
                return Err(format!(
                    "`manifestAreas` entry `{entry}` is not `languages/<name>` or \
                     `capabilities/<group>/<name>`"
                ));
            }
        };
        if !kind.iter().any(|seen| seen == entry) {
            kind.push(entry.to_owned());
        }
    }
    languages.extend(capabilities);
    match languages.iter().find(|area| !exists(files, area)) {
        Some(area) => Err(format!(
            "`manifestAreas` entry `{area}` does not exist in this release"
        )),
        None => Ok(languages),
    }
}

/// Whether the area folder exists under its parent folder.
fn exists(files: &impl ReleaseFiles, area: &str) -> bool {
    area.rsplit_once('/')
        .is_some_and(|(parent, name)| files.folders(parent).iter().any(|folder| folder == name))
}

/// Whether a language, group or capability name is lowercase, hyphenated and at most 64 characters.
fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

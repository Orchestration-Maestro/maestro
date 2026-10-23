//! Strict corpus boundary records.
use maestro_app::{PathMetadata, ResolvedPaths, ResolvedResource};
use serde::Deserialize;
use serde_json::Value;
/// A recorded resource with its provenance.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resource {
    /// Authored path.
    pub path: String,
    /// Initial enabled state.
    pub enabled: bool,
    /// Complete provenance.
    pub metadata: Metadata,
}
/// Strict provenance boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Metadata {
    /// Native scope representation.
    pub scope: maestro_app::SourceScope,
    /// Native origin representation.
    pub origin: maestro_app::SourceOrigin,
    /// Authored source.
    pub source: String,
    /// Missing differs from present empty.
    #[serde(rename = "baseDir")]
    pub base_dir: Option<String>,
}
impl Resource {
    /// Converts into the existing resolved owner.
    pub(super) fn resolved(self) -> ResolvedResource {
        ResolvedResource {
            path: self.path,
            enabled: self.enabled,
            metadata: self.metadata.native(),
        }
    }
}
impl Metadata {
    /// Keeps the optional base and every provenance field.
    pub(super) fn native(self) -> PathMetadata {
        PathMetadata {
            scope: self.scope,
            origin: self.origin,
            source: self.source,
            base_dir: self.base_dir,
        }
    }
}
/// The four existing resolved collections.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resolved {
    /// Extension files.
    pub extensions: Vec<Resource>,
    /// Skill files.
    pub skills: Vec<Resource>,
    /// Prompt files.
    pub prompts: Vec<Resource>,
    /// Theme files.
    pub themes: Vec<Resource>,
}
impl Resolved {
    /// Uses existing records without a frontend copy of their domain.
    pub(super) fn native(self) -> ResolvedPaths {
        ResolvedPaths {
            extensions: self
                .extensions
                .into_iter()
                .map(Resource::resolved)
                .collect(),
            skills: self.skills.into_iter().map(Resource::resolved).collect(),
            prompts: self.prompts.into_iter().map(Resource::resolved).collect(),
            themes: self.themes.into_iter().map(Resource::resolved).collect(),
            ..ResolvedPaths::default()
        }
    }
}
/// Initial raw documents for both scopes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scopes {
    /// User document.
    pub global: Value,
    /// Project document.
    pub project: Value,
}
/// Recorded constructor and operation sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    /// Supplied resolved inventory.
    pub resolved: Resolved,
    /// Supplied scope documents.
    pub settings: Scopes,
    /// Selected project directory, not ambient cwd.
    pub cwd: String,
    /// User directory.
    pub agent_dir: String,
    /// Subsequent operations.
    pub operations: Vec<Operation>,
    /// Viewport width.
    pub width: usize,
    /// Initial bold policy.
    pub styles_enabled: Option<bool>,
}
/// Operations admitted by the recorded public protocol.
#[derive(Deserialize)]
#[serde(
    tag = "name",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum Operation {
    /// Terminal input.
    Input(String),
    /// Enable or disable bold.
    Styles(bool),
    /// Publish shifted palette.
    Publish(u8),
    /// Set component focus.
    Focus(bool),
    /// Invalidate component children.
    Invalidate,
    /// Replace live bindings.
    Bindings(std::collections::BTreeMap<String, String>),
    /// Detach children while retaining the array.
    ClearChildren,
    /// Reinstall the original child array.
    RestoreChildren,
    /// Replace cancel/exit slots.
    ReplaceCancelExit,
    /// Remove all slots.
    ClearCallbacks,
    /// Install full toggle observer.
    InstallObserver,
    /// Install callback that synchronously re-enters the list.
    InstallReentrantObserver,
    /// Mutate every captured field.
    MutateCapture(Mutation),
    /// Update the captured shared item.
    UpdateCapture(bool),
    /// Update a detached item with selected identity.
    DetachedUpdate(Detached),
    /// Await queued writes and reload.
    Reload,
}
/// Full caller mutation of the captured live record.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Mutation {
    /// New display label.
    pub display_name: String,
    /// New path.
    pub path: String,
    /// New kind.
    pub resource_type: String,
    /// New enabled state.
    pub enabled: bool,
    /// New group key.
    pub group_key: String,
    /// New subgroup key.
    pub subgroup_key: String,
    /// New provenance.
    pub metadata: Metadata,
}
/// A detached update's distinguishable identity.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Detached {
    /// Path to match.
    pub path: String,
    /// Kind to match.
    pub resource_type: String,
    /// Requested state.
    pub enabled: bool,
}
/// Complete independently recorded observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Expected {
    /// Constructor and post-operation frames.
    pub frames: Vec<Frame>,
    /// Every operation outcome.
    pub outcomes: Vec<String>,
    /// Final stored documents.
    pub persisted: Scopes,
}
/// A complete styled frame and ordered settings/notification observation.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Frame {
    /// Member order for every package entry.
    pub package_keys: Value,
    /// Full component rows.
    pub lines: Vec<String>,
    /// Retained list rows.
    pub list_lines: Vec<String>,
    /// User snapshot.
    pub global: Value,
    /// Project snapshot.
    pub project: Value,
    /// User member order.
    pub global_keys: Vec<String>,
    /// Project member order.
    pub project_keys: Vec<String>,
    /// Full cumulative event transcript.
    pub events: Vec<Value>,
}

//! Ordered attributed resource-path contributions.
use super::{ExtensionRunner, dispatch::HandlerCursor};
use crate::{
    EventError, ExtensionExecution,
    types::{
        events::{ResourcesDiscoverEvent, ResourcesDiscoverReason, ResourcesDiscoverResult},
        read,
    },
};
use maestro_session::Presence;

/// One authored resource path and its current emitting extension path.
pub struct ResourcePath {
    /// Path supplied by the callback, without normalization.
    pub path: String,
    /// Current path of its registration owner.
    pub extension_path: String,
}
/// Independent resource lists retaining contribution order and duplicates.
pub struct ResourcesDiscoverCombinedResult {
    /// Skill contributions.
    pub skill_paths: Vec<ResourcePath>,
    /// Prompt contributions.
    pub prompt_paths: Vec<ResourcePath>,
    /// Theme contributions.
    pub theme_paths: Vec<ResourcePath>,
}
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Collects resource paths independently by kind, preserving attribution.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_resources_discover(
        &self,
        cwd: String,
        reason: ResourcesDiscoverReason,
    ) -> Result<ResourcesDiscoverCombinedResult, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut combined = ResourcesDiscoverCombinedResult {
            skill_paths: Vec::new(),
            prompt_paths: Vec::new(),
            theme_paths: Vec::new(),
        };
        while let Some(invocation) =
            cursor.next::<_, ResourcesDiscoverResult>(self, "resources_discover")
        {
            let mut event = ResourcesDiscoverEvent {
                cwd: cwd.clone(),
                reason,
            };
            if let Some(result) = invocation.call(self, &mut event, context.clone()).await? {
                let path = read(&invocation.owner).path.clone();
                append_paths(&mut combined.skill_paths, result.skill_paths, &path);
                append_paths(&mut combined.prompt_paths, result.prompt_paths, &path);
                append_paths(&mut combined.theme_paths, result.theme_paths, &path);
            }
        }
        Ok(combined)
    }
}
/// Appends only supplied lists, without sorting or deduplication.
fn append_paths(
    output: &mut Vec<ResourcePath>,
    paths: Presence<Vec<String>>,
    extension_path: &str,
) {
    if let Presence::Present(paths) = paths {
        output.extend(paths.into_iter().map(|path| ResourcePath {
            path,
            extension_path: extension_path.to_owned(),
        }));
    }
}

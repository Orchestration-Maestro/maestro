//! The detection-manifest commands: the retained catalogue's status and its reload.

use super::{super::App, responses::api_error};
use crate::paths::DaemonPaths;
use maestro_detection::{
    agents::agent_label,
    manifest::AgentManifestSummary,
    manifest_update::{ManifestUpdateStatus, load_status},
};
use maestro_protocol::schema::{AgentManifestInfo, ErrorBody, ResponseResult};
use std::path::Path;

impl<C: FnMut() -> u128> App<C> {
    /// Reports every active manifest and the last catalogue check, leaving detection as it is.
    pub(super) fn agent_manifest_status(&mut self, paths: &DaemonPaths) -> ResponseResult {
        self.state.agent_manifest_summaries = self
            .state
            .manifests
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .manifest_summaries();
        let status = load_status(Path::new(&paths.detection_catalog()));
        ResponseResult::AgentManifestStatus {
            last_check_unix: status.last_check_unix,
            last_result: status.last_result.clone(),
            manifests: manifest_infos(&self.state.agent_manifest_summaries, &status),
        }
    }

    /// Reloads the shared manifest catalogue and restarts detection in every live pane.
    ///
    /// # Errors
    /// Returns `internal_error` when a bundled manifest fails to load; the previous
    /// catalogue stays in use.
    pub(super) fn reload_agent_manifests(
        &mut self,
        paths: &DaemonPaths,
    ) -> Result<ResponseResult, ErrorBody> {
        let summaries = self
            .state
            .manifests
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reload_manifests()
            .map_err(|message| api_error("internal_error", message))?;
        let status = load_status(Path::new(&paths.detection_catalog()));
        for runtime in self.terminal_runtimes.values() {
            runtime.reset_agent_detection();
        }
        let manifests = manifest_infos(&summaries, &status);
        self.state.agent_manifest_summaries = summaries;
        Ok(ResponseResult::AgentManifestReload { manifests })
    }
}

/// The public record of each manifest with its agent's last remote update outcome.
fn manifest_infos(
    summaries: &[AgentManifestSummary],
    status: &ManifestUpdateStatus,
) -> Vec<AgentManifestInfo> {
    summaries
        .iter()
        .map(|summary| {
            let remote = status.agent_status(summary.agent);
            AgentManifestInfo {
                agent: agent_label(summary.agent).to_owned(),
                source: summary.active_source.label(),
                source_kind: summary.active_source.kind().to_owned(),
                active_version: summary.active_version.clone(),
                cached_remote_version: summary.cached_remote_version.clone(),
                local_override_shadowing_remote: summary.local_override_shadowing_remote,
                remote_update_result: remote.as_ref().map(|remote| remote.last_result.clone()),
                remote_update_error: remote.as_ref().and_then(|remote| remote.last_error.clone()),
                remote_last_checked_unix: remote.and_then(|remote| remote.last_checked_unix),
                warning: summary.warning.clone(),
            }
        })
        .collect()
}

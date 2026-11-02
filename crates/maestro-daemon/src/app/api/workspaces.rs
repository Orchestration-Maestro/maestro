//! Workspace commands: list, get, create, focus, rename, move, block moves, metadata and
//! close.

use super::{
    super::{
        App,
        api_helpers::{
            MAX_METADATA_TOKEN_KEYS_PER_RESOURCE, normalize_metadata_source,
            normalize_metadata_tokens, normalize_metadata_ttl,
        },
        ids::parse_positional_workspace_id,
    },
    env::normalize_launch_env,
    responses::{api_error, workspace_not_found},
};
use maestro_protocol::{
    events::EventData,
    schema::{
        ErrorBody, ResponseResult, WorkspaceCreateParams, WorkspaceInfo, WorkspaceMoveBlockParams,
        WorkspaceMoveParams, WorkspaceRenameParams, WorkspaceReportMetadataParams,
    },
};
use maestro_workspace::metadata_tokens::{MAX_SEQUENCE_SOURCES, accept_sequence};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

impl<C: FnMut() -> u128> App<C> {
    /// Lists every workspace in display order.
    pub(super) fn handle_workspace_list(&self) -> ResponseResult {
        ResponseResult::WorkspaceList {
            workspaces: self.workspace_list_info(),
        }
    }

    /// Reads one workspace.
    pub(super) fn handle_workspace_get(&self, id: &str) -> Result<ResponseResult, ErrorBody> {
        let index = self.target_workspace(id)?;
        self.workspace_info_result(index, id)
    }

    /// Creates a workspace in the supplied or followed directory and publishes its records.
    pub(in crate::app) async fn handle_workspace_create(
        &mut self,
        params: WorkspaceCreateParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let cwd = match params.cwd {
            Some(cwd) => PathBuf::from(cwd),
            None => self.new_workspace_cwd().await,
        };
        let extra_env = normalize_launch_env(params.env)?;
        let index = self
            .create_workspace_with_launch_env(cwd, params.focus, extra_env)
            .map_err(|error| api_error("workspace_create_failed", error.to_string()))?;
        if let Some(label) = params.label {
            let workspace = &mut self.state.topology.workspaces[index];
            workspace.set_custom_name(label);
            crate::logging::workspace_renamed(&workspace.id);
        }
        self.emit_workspace_open_events(index).await;
        self.workspace_created_result(index)
            .await
            .ok_or_else(|| api_error("workspace_create_failed", "workspace disappeared"))
    }

    /// Activates one workspace.
    pub(super) fn handle_workspace_focus(&mut self, id: &str) -> Result<ResponseResult, ErrorBody> {
        let index = self.target_workspace(id)?;
        self.state.switch_workspace(index);
        self.workspace_info_result(index, id)
    }

    /// Names one workspace and publishes the new label.
    pub(super) fn handle_workspace_rename(
        &mut self,
        params: WorkspaceRenameParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let index = self.target_workspace(&params.workspace_id)?;
        let workspace = &mut self.state.topology.workspaces[index];
        workspace.set_custom_name(params.label.clone());
        crate::logging::workspace_renamed(&workspace.id);
        let workspace_id = workspace.id.clone();
        self.state.session_dirty = true;
        self.event_hub.push(EventData::WorkspaceRenamed {
            workspace_id,
            label: params.label,
        });
        self.workspace_info_result(index, &params.workspace_id)
    }

    /// Moves one workspace before an insertion position; only a real change is published.
    pub(super) fn handle_workspace_move(
        &mut self,
        params: &WorkspaceMoveParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let index = self.target_workspace(&params.workspace_id)?;
        if params.insert_index > self.state.topology.workspaces.len() {
            return Err(api_error(
                "workspace_move_failed",
                format!("insert_index {} is out of bounds", params.insert_index),
            ));
        }
        let workspace_id = self.state.topology.workspaces[index].id.clone();
        let moved = self.state.move_workspace(index, params.insert_index);
        let workspaces = self.workspace_list_info();
        if moved {
            self.event_hub.push(EventData::WorkspaceMoved {
                workspace_id,
                insert_index: params.insert_index,
                workspaces: workspaces.clone(),
            });
        }
        Ok(ResponseResult::WorkspaceList { workspaces })
    }

    /// Moves a block of workspaces, in the caller's order, before another one or to the end.
    ///
    /// Every member and the anchor resolve through one index of this request, so the work
    /// stays linear in the number of workspaces.
    pub(super) fn handle_workspace_move_block(
        &mut self,
        params: &WorkspaceMoveBlockParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let failed = |message: String| api_error("workspace_move_block_failed", message);
        if params.workspace_ids.is_empty() {
            return Err(failed("workspace_ids must not be empty".into()));
        }
        let workspaces = &self.state.topology.workspaces;
        let positions: HashMap<&str, usize> = workspaces
            .iter()
            .enumerate()
            .map(|(index, workspace)| {
                #[cfg(test)]
                crate::tests::record_workspace_work();
                (workspace.id.as_str(), index)
            })
            .collect();
        let resolve = |requested: &str| {
            #[cfg(test)]
            crate::tests::record_workspace_work();
            positions
                .get(requested)
                .copied()
                .or_else(|| parse_positional_workspace_id(requested))
                .and_then(|index| workspaces.get(index))
                .map(|workspace| workspace.id.clone())
                .ok_or_else(|| workspace_not_found(requested))
        };
        let mut workspace_ids = Vec::with_capacity(params.workspace_ids.len());
        let mut seen = HashSet::with_capacity(params.workspace_ids.len());
        for input in &params.workspace_ids {
            let id = resolve(input)?;
            if !seen.insert(id.clone()) {
                return Err(failed(format!("workspace {input} appears more than once")));
            }
            workspace_ids.push(id);
        }
        let before_workspace_id = params
            .before_workspace_id
            .as_deref()
            .map(resolve)
            .transpose()?;
        if before_workspace_id
            .as_ref()
            .is_some_and(|id| seen.contains(id))
        {
            return Err(failed(
                "before_workspace_id must not be part of workspace_ids".into(),
            ));
        }
        let moved = self
            .state
            .move_workspace_block(&workspace_ids, before_workspace_id.as_deref());
        let workspaces = self.workspace_list_info();
        if moved {
            self.event_hub.push(EventData::WorkspaceReordered {
                workspace_ids,
                before_workspace_id,
                workspaces: workspaces.clone(),
            });
        }
        Ok(ResponseResult::WorkspaceList { workspaces })
    }

    /// Patches a workspace's metadata tokens for one reporter, in sequence order.
    ///
    /// The target, then the source, lifetime and tokens are checked before anything
    /// changes; a stale sequence is accepted without effect.
    pub(super) fn handle_workspace_report_metadata(
        &mut self,
        params: WorkspaceReportMetadataParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let not_found = || workspace_not_found(&params.workspace_id);
        let index = self
            .state
            .parse_workspace_id(&params.workspace_id)
            .ok_or_else(not_found)?;
        let source = normalize_metadata_source(&params.source)
            .map_err(|message| api_error("invalid_metadata_source", message))?;
        let ttl = normalize_metadata_ttl(params.ttl_ms)
            .map_err(|message| api_error("invalid_metadata_ttl", message))?;
        let tokens = normalize_metadata_tokens(params.tokens)
            .map_err(|message| api_error("invalid_metadata_token", message))?;
        let workspace = self
            .state
            .topology
            .workspaces
            .get_mut(index)
            .ok_or_else(not_found)?;
        if !maestro_workspace::metadata_tokens::sequence_is_fresh(
            &workspace.metadata_token_sequences,
            &source,
            params.seq,
        ) {
            return Ok(ResponseResult::Ok {});
        }
        if workspace.metadata_tokens.key_count_after_patch(&tokens)
            > MAX_METADATA_TOKEN_KEYS_PER_RESOURCE
        {
            return Err(api_error(
                "metadata_token_limit",
                format!(
                    "workspace metadata may contain at most {MAX_METADATA_TOKEN_KEYS_PER_RESOURCE} tokens"
                ),
            ));
        }
        match accept_sequence(&mut workspace.metadata_token_sequences, &source, params.seq) {
            Ok(true) => {}
            Ok(false) => return Ok(ResponseResult::Ok {}),
            Err(_) => {
                return Err(api_error(
                    "metadata_sequence_source_limit",
                    format!(
                        "workspace metadata may track at most {MAX_SEQUENCE_SOURCES} sequenced sources"
                    ),
                ));
            }
        }
        if workspace
            .metadata_tokens
            .patch(tokens, ttl, std::time::Instant::now())
        {
            self.emit_workspace_metadata_updated(index);
        }
        Ok(ResponseResult::Ok {})
    }

    /// Closes one workspace with its owned terminals and publishes its final record.
    pub(super) async fn handle_workspace_close(
        &mut self,
        id: &str,
    ) -> Result<ResponseResult, ErrorBody> {
        let index = self.target_workspace(id)?;
        let workspace_id = self.state.topology.workspaces[index].id.clone();
        let workspace = self.workspace_info(index);
        self.state.selected = index;
        self.state.close_selected_workspace();
        self.shutdown_detached_terminal_runtimes().await;
        self.event_hub.push(EventData::WorkspaceClosed {
            workspace_id,
            workspace,
        });
        Ok(ResponseResult::Ok {})
    }

    /// Publishes the current record of a workspace whose metadata changed.
    pub(in crate::app) fn emit_workspace_metadata_updated(&self, index: usize) {
        if let Some(workspace) = self.workspace_info(index) {
            self.event_hub
                .push(EventData::WorkspaceMetadataUpdated { workspace });
        }
    }

    /// Resolves a workspace target to a live display position.
    pub(super) fn target_workspace(&self, id: &str) -> Result<usize, ErrorBody> {
        self.state
            .parse_workspace_id(id)
            .filter(|index| *index < self.state.topology.workspaces.len())
            .ok_or_else(|| workspace_not_found(id))
    }

    /// The record of a live workspace as a command result.
    fn workspace_info_result(&self, index: usize, id: &str) -> Result<ResponseResult, ErrorBody> {
        self.workspace_info(index)
            .map(|workspace| ResponseResult::WorkspaceInfo { workspace })
            .ok_or_else(|| workspace_not_found(id))
    }

    /// Every workspace's record in display order.
    fn workspace_list_info(&self) -> Vec<WorkspaceInfo> {
        (0..self.state.topology.workspaces.len())
            .filter_map(|index| self.workspace_info(index))
            .collect()
    }
}

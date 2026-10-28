//! Agent reports: state, session identity, authority clearing and release.

use super::super::responses::invalid_agent;
use crate::{
    agent_resume::{normalize_session_start_source, session_ref_from_report},
    app::{App, api_helpers::normalize_reported_agent_label},
    events::AppEvent,
    terminal::state::SessionStartReport,
};
use maestro_detection::agents::normalize_agent_source;
use maestro_protocol::schema::{
    ErrorBody, PaneClearAgentAuthorityParams, PaneReleaseAgentParams, PaneReportAgentParams,
    PaneReportAgentSessionParams, ResponseResult,
};

/// A reporter's agent label in its catalogue spelling, or the refusal of a blank one.
pub(super) fn reported_agent_label(agent: &str) -> Result<String, ErrorBody> {
    normalize_reported_agent_label(agent).ok_or_else(invalid_agent)
}

impl<C: FnMut() -> u128> App<C> {
    /// Applies an integration's reported state, with its session identity when it is an
    /// official source.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane that is gone and `invalid_agent` for a blank label.
    pub(super) fn handle_pane_report_agent(
        &mut self,
        params: PaneReportAgentParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (_, pane_id) = self.target_pane(&params.pane_id)?;
        let agent_label = reported_agent_label(&params.agent)?;
        let source = normalize_agent_source(params.source);
        self.handle_internal_event(AppEvent::HookStateReported {
            pane_id,
            session_ref: session_ref_from_report(
                &source,
                &agent_label,
                params.agent_session_id,
                params.agent_session_path,
            ),
            source,
            agent_label,
            state: params.state,
            message: params.message,
            seq: params.seq,
        });
        Ok(ResponseResult::Ok {})
    }

    /// Records an integration's session identity without giving it state authority.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane that is gone and `invalid_agent` for a blank label.
    pub(super) fn handle_pane_report_agent_session(
        &mut self,
        params: PaneReportAgentSessionParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (_, pane_id) = self.target_pane(&params.pane_id)?;
        let agent_label = reported_agent_label(&params.agent)?;
        let source = normalize_agent_source(params.source);
        self.handle_internal_event(AppEvent::AgentSessionReported {
            pane_id,
            report: SessionStartReport {
                session_ref: session_ref_from_report(
                    &source,
                    &agent_label,
                    params.agent_session_id,
                    params.agent_session_path,
                ),
                source,
                agent_label,
                seq: params.seq,
                session_start_source: normalize_session_start_source(params.session_start_source),
            },
        });
        Ok(ResponseResult::Ok {})
    }

    /// Withdraws the named integration's authority over the pane, or any integration's.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane that is gone.
    pub(super) fn handle_pane_clear_agent_authority(
        &mut self,
        params: PaneClearAgentAuthorityParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (_, pane_id) = self.target_pane(&params.pane_id)?;
        self.handle_internal_event(AppEvent::HookAuthorityCleared {
            pane_id,
            source: params.source,
            seq: params.seq,
        });
        Ok(ResponseResult::Ok {})
    }

    /// Returns the pane to the shell on an integration's say, keeping a live official agent.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane that is gone and `invalid_agent` for a blank label.
    pub(super) fn handle_pane_release_agent(
        &mut self,
        params: PaneReleaseAgentParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (_, pane_id) = self.target_pane(&params.pane_id)?;
        let agent_label = reported_agent_label(&params.agent)?;
        self.handle_internal_event(AppEvent::HookAgentReleased {
            pane_id,
            source: normalize_agent_source(params.source),
            known_agent: maestro_detection::agents::parse_agent_label(&agent_label),
            agent_label,
            seq: params.seq,
        });
        Ok(ResponseResult::Ok {})
    }
}

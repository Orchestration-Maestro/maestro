//! `pane.report_metadata`: presentation overrides and tokens a reporter attaches to a pane.

use super::{
    super::responses::{api_error, pane_not_found},
    reports::reported_agent_label,
};
use crate::{
    app::{
        App,
        api_helpers::{
            MAX_METADATA_TOKEN_KEYS_PER_RESOURCE, normalize_metadata_source,
            normalize_metadata_tokens, normalize_metadata_ttl,
        },
    },
    events::AppEvent,
    terminal::state::{AgentMetadataReport, TerminalState},
};
use maestro_protocol::schema::{ErrorBody, PaneReportMetadataParams, ResponseResult};
use maestro_workspace::metadata_tokens::MAX_SEQUENCE_SOURCES;
use std::{collections::HashMap, time::Instant};

/// Longest kept presentation text, in Unicode scalars.
const PRESENTATION_MAX_CHARS: usize = 80;

/// The execution states a reporter may label.
const STATE_LABELS: [&str; 5] = ["idle", "working", "blocked", "done", "unknown"];

/// A report whose fields all passed their checks.
struct ValidReport {
    /// The presentation update, with the sequence the report named.
    presentation: AgentMetadataReport,
    /// The token patch, when the report carries one.
    tokens: Option<HashMap<String, Option<String>>>,
}

impl ValidReport {
    /// Whether the report touches the pane's title, display agent or state labels.
    fn changes_presentation(&self) -> bool {
        let report = &self.presentation;
        report.title.is_some()
            || report.display_agent.is_some()
            || !report.state_labels.is_empty()
            || report.clear_title
            || report.clear_display_agent
            || report.clear_state_labels
    }
}

/// Drops control characters, trims, and keeps at most 80 scalars; blank text is absent.
fn normalize_presentation_text(value: Option<String>) -> Option<String> {
    let kept: String = value?
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(PRESENTATION_MAX_CHARS)
        .collect();
    let kept = kept.trim();
    (!kept.is_empty()).then(|| kept.to_owned())
}

/// Cleans each label and its state name, refusing a name that is not an execution state.
fn normalize_state_labels(
    labels: HashMap<String, String>,
) -> Result<HashMap<String, String>, ErrorBody> {
    let mut kept = HashMap::new();
    for (state, label) in labels {
        let state = state.trim().to_ascii_lowercase();
        if !STATE_LABELS.contains(&state.as_str()) {
            return Err(api_error(
                "invalid_state_label",
                format!("unknown state label: {state}"),
            ));
        }
        if let Some(label) = normalize_presentation_text(Some(label)) {
            kept.insert(state, label);
        }
    }
    Ok(kept)
}

/// Refuses a report that sets and clears one field, or that asks for nothing.
fn check_request(report: &ValidReport, set: [bool; 3]) -> Result<(), ErrorBody> {
    let presentation = &report.presentation;
    let [title, display_agent, state_labels] = set;
    if title && presentation.clear_title
        || display_agent && presentation.clear_display_agent
        || state_labels && presentation.clear_state_labels
    {
        return Err(api_error(
            "invalid_metadata_request",
            "cannot set and clear the same metadata field",
        ));
    }
    if !report.changes_presentation() && report.tokens.is_none() {
        return Err(api_error(
            "invalid_metadata_request",
            "missing metadata field to set or clear",
        ));
    }
    Ok(())
}

/// Checks and cleans every field of a report, in the order a reporter is told about them.
fn validate(params: PaneReportMetadataParams) -> Result<ValidReport, ErrorBody> {
    let agent_label = params
        .agent
        .as_deref()
        .map(reported_agent_label)
        .transpose()?;
    let source = normalize_metadata_source(&params.source)
        .map_err(|message| api_error("invalid_metadata_source", message))?;
    let set = [
        params.title.is_some(),
        params.display_agent.is_some(),
        !params.state_labels.is_empty(),
    ];
    let tokens = if params.tokens.is_empty() {
        None
    } else {
        Some(
            normalize_metadata_tokens(params.tokens)
                .map_err(|message| api_error("invalid_metadata_token", message))?,
        )
    };
    let ttl = normalize_metadata_ttl(params.ttl_ms)
        .map_err(|message| api_error("invalid_metadata_ttl", message))?;
    let applies_to_source = params
        .applies_to_source
        .as_deref()
        .map(normalize_metadata_source)
        .transpose()
        .map_err(|message| api_error("invalid_metadata_source", message))?;
    let report = ValidReport {
        presentation: AgentMetadataReport {
            source,
            agent_label,
            applies_to_source,
            title: normalize_presentation_text(params.title),
            display_agent: normalize_presentation_text(params.display_agent),
            state_labels: normalize_state_labels(params.state_labels)?,
            clear_title: params.clear_title,
            clear_display_agent: params.clear_display_agent,
            clear_state_labels: params.clear_state_labels,
            ttl,
            seq: params.seq,
        },
        tokens,
    };
    check_request(&report, set)?;
    Ok(report)
}

/// Admits the report's sequence for its source, ignoring one that is stale or follows the
/// agent's exit, and refusing one that would overfill the pane's token or source limits.
///
/// Returns whether the report goes on to apply.
fn admit(terminal: &mut TerminalState, report: &ValidReport) -> Result<bool, ErrorBody> {
    let presentation = &report.presentation;
    let (agent, guard) = (
        presentation.agent_label.as_deref(),
        presentation.applies_to_source.as_deref(),
    );
    if terminal.metadata_report_blocked_by_process_exit(&presentation.source, agent, guard)
        || !terminal.metadata_report_sequence_is_fresh(&presentation.source, presentation.seq)
    {
        return Ok(false);
    }
    if let Some(tokens) = &report.tokens
        && terminal.metadata_tokens.key_count_after_patch(tokens)
            > MAX_METADATA_TOKEN_KEYS_PER_RESOURCE
    {
        return Err(api_error(
            "metadata_token_limit",
            format!(
                "pane metadata may contain at most {MAX_METADATA_TOKEN_KEYS_PER_RESOURCE} tokens"
            ),
        ));
    }
    let owner = TerminalState::metadata_report_agent(&presentation.source, agent, guard);
    terminal
        .accept_metadata_report(
            &presentation.source,
            presentation.seq,
            report.tokens.is_some(),
            owner,
        )
        .map_err(|()| {
            api_error(
                "metadata_sequence_source_limit",
                format!("pane metadata may track at most {MAX_SEQUENCE_SOURCES} sequenced sources"),
            )
        })
}

impl<C: FnMut() -> u128> App<C> {
    /// Applies a reporter's presentation overrides and token patch to the named pane.
    ///
    /// A stale sequence, or a report that follows the exit of the agent it names, is
    /// accepted without effect.
    ///
    /// # Errors
    /// Returns `pane_not_found`, `invalid_agent`, `invalid_metadata_source`,
    /// `invalid_metadata_token`, `invalid_metadata_ttl`, `invalid_state_label`,
    /// `invalid_metadata_request`, `metadata_token_limit` or `metadata_sequence_source_limit`.
    pub(super) async fn handle_pane_report_metadata(
        &mut self,
        params: PaneReportMetadataParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.target_pane(&params.pane_id)?;
        let pane_id = params.pane_id.clone();
        let report = validate(params)?;
        let terminal = self.state.topology.workspaces[workspace]
            .terminal_id(pane)
            .and_then(|id| self.state.terminals.get_mut(id))
            .ok_or_else(|| pane_not_found(&pane_id))?;
        if !admit(terminal, &report)? {
            return Ok(ResponseResult::Ok {});
        }
        let changes_presentation = report.changes_presentation();
        let ValidReport {
            presentation,
            tokens,
        } = report;
        let tokens_changed = tokens.is_some_and(|tokens| {
            let changed = terminal
                .metadata_tokens
                .patch(tokens, presentation.ttl, Instant::now());
            if changed {
                terminal.revision = terminal.revision.saturating_add(1);
            }
            changed
        });
        if changes_presentation {
            self.handle_internal_event(AppEvent::HookMetadataReported {
                pane_id: pane,
                report: AgentMetadataReport {
                    seq: None,
                    ..presentation
                },
            });
        }
        if tokens_changed {
            self.emit_pane_updated(workspace, pane).await;
        }
        Ok(ResponseResult::Ok {})
    }
}

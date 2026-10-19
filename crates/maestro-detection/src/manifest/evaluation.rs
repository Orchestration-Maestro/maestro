//! Rule selection, explain evidence and JSON projection.
use super::{
    Agent, AgentDetection, AgentState, AgentVisibility, CompiledGate, CompiledRule,
    DEFAULT_KNOWN_AGENT_IDLE_FALLBACK, DetectionExplain, DetectionInput, EvaluatedRule,
    LoadedManifest, ManifestRule, ManifestSource, MatchedRule, RuleEvidence, agent_label, region,
};
impl DetectionExplain {
    /// Projects explain output into live state and visibility metadata.
    pub(super) fn into_detection(self) -> AgentDetection {
        AgentDetection {
            state: self.state,
            skip_state_update: self.skip_state_update,
            visible: self.visible,
        }
    }
}
/// Selects the first highest-priority matching rule and retains every evaluation.
pub(super) fn evaluate_loaded_manifest(
    agent: Agent,
    input: DetectionInput<'_>,
    loaded: &LoadedManifest,
    update_status: Option<crate::manifest_update::AgentRemoteStatus>,
) -> DetectionExplain {
    let mut matched: Option<&ManifestRule> = None;
    let mut evaluated_rules = Vec::new();

    for (rule, compiled_rule) in loaded.manifest.rules.iter().zip(&loaded.compiled_rules) {
        let region_text = region(input, &rule.region);
        let matched_rule = compiled_rule_matches(compiled_rule, region_text);
        evaluated_rules.push(EvaluatedRule {
            id: rule.id.clone(),
            priority: rule.priority,
            region: rule.region.clone(),
            evidence: rule_evidence(rule, region_text),
            state: rule.state.map_or(AgentState::Unknown, AgentState::from),
            matched: matched_rule,
        });

        if !matched_rule {
            continue;
        }

        match matched {
            Some(previous) if previous.priority >= rule.priority => {}
            _ => matched = Some(rule),
        }
    }

    let Some(rule) = matched else {
        return fallback_explain(Some(agent), Some((loaded, evaluated_rules)), update_status);
    };

    let state = rule.state.map_or(AgentState::Unknown, AgentState::from);
    let skipped_update_reason = rule
        .skip_state_update
        .then(|| format!("matched_rule:{}", rule.id));

    let remote_update_status = update_status;

    DetectionExplain {
        agent: Some(agent_label(agent).to_string()),
        state,
        source: Some(loaded.source.clone()),
        matched_rule: Some(MatchedRule {
            id: rule.id.clone(),
            priority: rule.priority,
            region: rule.region.clone(),
            state,
        }),
        screen_detection_skipped: false,
        visible: AgentVisibility {
            visible_idle: rule.visible.visible_idle && state == AgentState::Idle,
            visible_blocker: rule.visible.visible_blocker && state == AgentState::Blocked,
            visible_working: rule.visible.visible_working && state == AgentState::Working,
        },
        skip_state_update: rule.skip_state_update,
        skipped_update_reason,
        fallback_reason: None,
        evaluated_rules,
        warning: loaded.warning.clone(),
        manifest_version: loaded.manifest.version.as_ref().map(ToString::to_string),
        cached_remote_version: loaded.cached_remote_version.clone(),
        local_override_shadowing_remote: loaded.local_override_shadowing_remote,
        remote_update_status: remote_update_status
            .as_ref()
            .map(|status| status.last_result.clone()),
        remote_update_error: remote_update_status.and_then(|status| status.last_error),
    }
}
/// Preserves active-source metadata when no rule supplies live evidence.
pub(super) fn fallback_explain(
    agent: Option<Agent>,
    context: Option<(&LoadedManifest, Vec<EvaluatedRule>)>,
    update_status: Option<crate::manifest_update::AgentRemoteStatus>,
) -> DetectionExplain {
    let (
        source,
        evaluated_rules,
        warning,
        manifest_version,
        cached_remote_version,
        local_override_shadowing_remote,
    ) = context.map_or(
        (None, Vec::new(), None, None, None, false),
        |(loaded, evaluated)| {
            (
                Some(loaded.source.clone()),
                evaluated,
                loaded.warning.clone(),
                loaded.manifest.version.as_ref().map(ToString::to_string),
                loaded.cached_remote_version.clone(),
                loaded.local_override_shadowing_remote,
            )
        },
    );
    let known_agent = agent.is_some();
    let remote_update_status = update_status;

    DetectionExplain {
        agent: agent.map(|agent| agent_label(agent).to_string()),
        state: if known_agent {
            AgentState::Idle
        } else {
            AgentState::Unknown
        },
        source,
        matched_rule: None,
        screen_detection_skipped: false,
        visible: AgentVisibility::default(),
        skip_state_update: false,
        skipped_update_reason: None,
        fallback_reason: known_agent.then(|| DEFAULT_KNOWN_AGENT_IDLE_FALLBACK.to_string()),
        evaluated_rules,
        warning,
        manifest_version,
        cached_remote_version,
        local_override_shadowing_remote,
        remote_update_status: remote_update_status
            .as_ref()
            .map(|status| status.last_result.clone()),
        remote_update_error: remote_update_status.and_then(|status| status.last_error),
    }
}
/// Returns the lowercase state spelling used in explain JSON.
#[must_use]
pub fn agent_state_label(state: AgentState) -> &'static str {
    match state {
        AgentState::Idle => "idle",
        AgentState::Working => "working",
        AgentState::Blocked => "blocked",
        AgentState::Unknown => "unknown",
    }
}
/// Projects all explain fields, including absent values, into JSON.
#[must_use]
pub fn explain_to_json_value(explain: &DetectionExplain) -> serde_json::Value {
    let matched_rule = explain.matched_rule.as_ref().map(|rule| {
        serde_json::json!({
            "id": rule.id,
            "priority": rule.priority,
            "region": rule.region,
            "state": agent_state_label(rule.state),
        })
    });
    let evaluated_rules: Vec<_> = explain
        .evaluated_rules
        .iter()
        .map(|rule| {
            serde_json::json!({
                "id": rule.id,
                "priority": rule.priority,
                "region": rule.region,
                "state": agent_state_label(rule.state),
                "matched": rule.matched,
                "evidence": {
                    "contains": &rule.evidence.contains,
                    "regex": &rule.evidence.regex,
                    "line_regex": &rule.evidence.line_regex,
                    "all_count": rule.evidence.all_count,
                    "any_count": rule.evidence.any_count,
                    "not_count": rule.evidence.not_count,
                    "region_bytes": rule.evidence.region_bytes,
                    "region_preview": &rule.evidence.region_preview,
                },
            })
        })
        .collect();

    serde_json::json!({
        "agent": explain.agent,
        "state": agent_state_label(explain.state),
        "manifest_source": explain.source.as_ref().map(ManifestSource::label),
        "manifest_version": &explain.manifest_version,
        "cached_remote_version": &explain.cached_remote_version,
        "local_override_shadowing_remote": explain.local_override_shadowing_remote,
        "remote_update_status": &explain.remote_update_status,
        "remote_update_error": &explain.remote_update_error,
        "matched_rule": matched_rule,
        "visible_idle": explain.visible.visible_idle,
        "visible_blocker": explain.visible.visible_blocker,
        "visible_working": explain.visible.visible_working,
        "screen_detection_skipped": explain.screen_detection_skipped,
        "skip_state_update": explain.skip_state_update,
        "skipped_update_reason": explain.skipped_update_reason,
        "fallback_reason": explain.fallback_reason,
        "warning": explain.warning,
        "evaluated_rules": evaluated_rules,
    })
}
/// Matches a retained rule against its selected region.
fn compiled_rule_matches(rule: &CompiledRule, text: &str) -> bool {
    let lower_text = text.to_lowercase();
    compiled_gate_matches(&rule.gate, text, &lower_text)
}
/// Copies authored matcher metadata into owned explain output.
fn rule_evidence(rule: &ManifestRule, region_text: &str) -> RuleEvidence {
    RuleEvidence {
        contains: rule.contains.clone(),
        regex: rule.regex.clone(),
        line_regex: rule.line_regex.clone(),
        all_count: rule.all.len(),
        any_count: rule.any.len(),
        not_count: rule.not_gate.len(),
        region_bytes: region_text.len(),
        region_preview: bounded_preview(region_text),
    }
}
/// Truncates evidence only after 240 Unicode scalars.
fn bounded_preview(text: &str) -> String {
    const MAX_CHARS: usize = 240;
    let mut preview: String = text.chars().take(MAX_CHARS).collect();
    if text.chars().count() > MAX_CHARS {
        preview.push_str("...");
    }
    preview
}
/// Evaluates direct, conjunctive, alternative and exclusion matchers.
fn compiled_gate_matches(gate: &CompiledGate, text: &str, lower_text: &str) -> bool {
    if !gate
        .contains
        .iter()
        .all(|needle| lower_text.contains(needle))
    {
        return false;
    }

    if !gate.regex.iter().all(|regex| regex.is_match(text)) {
        return false;
    }

    if !gate
        .line_regex
        .iter()
        .all(|regex| text.lines().any(|line| regex.is_match(line)))
    {
        return false;
    }

    if !gate
        .all
        .iter()
        .all(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    if !gate.any.is_empty()
        && !gate
            .any
            .iter()
            .any(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    if gate
        .not_gate
        .iter()
        .any(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    true
}

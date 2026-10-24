//! Shape, complexity, region and pattern validation.
use super::{
    Agent, AgentManifest, CompiledGate, CompiledRule, MAX_GATE_DEPTH, MAX_MATCHER_CHARS,
    MAX_MATCHERS_PER_GATE, MAX_RULES_PER_MANIFEST, MAX_TOTAL_GATES, MAX_TOTAL_MATCHERS,
    ManifestGate, ManifestRule, ManifestState, ManifestVersion, Regex,
    TOP_NON_EMPTY_LINES_ENGINE_VERSION, agent_label, parse_agent_label, region_count,
    top_region_count,
};
/// Validated downloaded document with its required version.
pub struct ParsedRemoteManifest {
    /// Validated authored document retained for explain metadata.
    pub manifest: AgentManifest,
    /// Validated declared document version.
    pub version: ManifestVersion,
}
/// Parses and validates a detection document.
///
/// # Errors
/// Returns TOML, matcher, region or complexity diagnostics for invalid documents.
pub fn parse_manifest(content: &str) -> Result<AgentManifest, String> {
    let manifest = toml::from_str::<AgentManifest>(content).map_err(|err| err.to_string())?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}
/// Validates downloaded identity, version and engine compatibility.
///
/// # Errors
/// Returns document errors or a missing/incompatible identity or version diagnostic.
pub fn parse_remote_manifest_for_agent(
    agent: Agent,
    content: &str,
) -> Result<ParsedRemoteManifest, String> {
    let manifest = parse_manifest(content)?;
    if !manifest_matches_agent(&manifest, agent) {
        return Err(format!(
            "manifest id {} does not match {}",
            manifest.id,
            agent_label(agent)
        ));
    }
    let version = manifest
        .version
        .clone()
        .ok_or("remote manifest must include version")?;
    let min_engine_version = manifest
        .min_engine_version
        .ok_or("remote manifest must include min_engine_version")?;
    if min_engine_version > crate::updates::MANIFEST_ENGINE_VERSION {
        return Err(format!(
            "manifest requires engine {min_engine_version}, current engine is {}",
            crate::updates::MANIFEST_ENGINE_VERSION
        ));
    }
    Ok(ParsedRemoteManifest { manifest, version })
}
/// Checks rule metadata and aggregate matcher complexity.
fn validate_manifest(manifest: &AgentManifest) -> Result<(), String> {
    if manifest.rules.is_empty() {
        return Err("manifest must contain at least one rule".to_string());
    }
    if manifest.rules.len() > MAX_RULES_PER_MANIFEST {
        return Err(format!(
            "manifest contains {} rules, max is {MAX_RULES_PER_MANIFEST}",
            manifest.rules.len()
        ));
    }

    let mut complexity = ManifestComplexity::default();
    for rule in &manifest.rules {
        if rule.id.trim().is_empty() {
            return Err("manifest rule id must not be empty".to_string());
        }
        if rule.skip_state_update {
            if rule.state != Some(ManifestState::Unknown) {
                return Err(format!(
                    "rule {} uses skip_state_update without state = \"unknown\"",
                    rule.id
                ));
            }
            if rule.visible.visible_idle
                || rule.visible.visible_blocker
                || rule.visible.visible_working
            {
                return Err(format!(
                    "rule {} uses skip_state_update with visible state evidence",
                    rule.id
                ));
            }
        }
        validate_region_name(&rule.region)
            .map_err(|err| format!("rule {} uses invalid region: {err}", rule.id))?;
        if rule.region.trim().starts_with("top_non_empty_lines(")
            && manifest
                .min_engine_version
                .is_some_and(|version| version < TOP_NON_EMPTY_LINES_ENGINE_VERSION)
        {
            return Err(format!(
                "rule {} uses top_non_empty_lines but min_engine_version is below {}",
                rule.id, TOP_NON_EMPTY_LINES_ENGINE_VERSION
            ));
        }
        validate_rule_gate(rule, &mut complexity)
            .map_err(|err| format!("rule {} has invalid matcher gates: {err}", rule.id))?;
    }

    Ok(())
}
/// Aggregate gate and matcher counts accumulated during validation.
#[derive(Default)]
struct ManifestComplexity {
    /// Root and nested gates visited so far.
    total_gates: usize,
    /// Direct matchers visited across every gate.
    total_matchers: usize,
}
/// Validates the borrowed root matcher tree of a rule.
fn validate_rule_gate(
    rule: &ManifestRule,
    complexity: &mut ManifestComplexity,
) -> Result<(), String> {
    validate_gate(manifest_gate_from_rule(rule), "rule", 0, complexity)
}
/// Checks positive gates and their bounded recursive descendants.
fn validate_gate(
    gate: GateView<'_>,
    context: &str,
    depth: usize,
    complexity: &mut ManifestComplexity,
) -> Result<(), String> {
    if depth > MAX_GATE_DEPTH {
        return Err(format!("{context} exceeds max gate depth {MAX_GATE_DEPTH}"));
    }
    complexity.total_gates += 1;
    if complexity.total_gates > MAX_TOTAL_GATES {
        return Err(format!("manifest exceeds max gate count {MAX_TOTAL_GATES}"));
    }
    validate_matcher_limits(gate, context, complexity)?;
    if !gate_has_positive_matcher(gate) {
        return Err(format!("{context} must contain a positive matcher"));
    }
    validate_regex_patterns(gate.regex, context, "regex")?;
    validate_regex_patterns(gate.line_regex, context, "line_regex")?;
    for nested in gate.all {
        validate_gate(nested.into(), "all gate", depth + 1, complexity)?;
    }
    for nested in gate.any {
        validate_gate(nested.into(), "any gate", depth + 1, complexity)?;
    }
    for nested in gate.not_gate {
        if !gate_has_any_matcher(nested.into()) {
            return Err(format!("{context} contains an empty not gate"));
        }
        validate_not_gate(nested.into(), depth + 1, complexity)?;
    }
    Ok(())
}
/// Checks exclusion gates, including nested exclusions.
fn validate_not_gate(
    gate: GateView<'_>,
    depth: usize,
    complexity: &mut ManifestComplexity,
) -> Result<(), String> {
    if depth > MAX_GATE_DEPTH {
        return Err(format!("not gate exceeds max gate depth {MAX_GATE_DEPTH}"));
    }
    complexity.total_gates += 1;
    if complexity.total_gates > MAX_TOTAL_GATES {
        return Err(format!("manifest exceeds max gate count {MAX_TOTAL_GATES}"));
    }
    validate_matcher_limits(gate, "not gate", complexity)?;
    if !gate_has_any_matcher(gate) {
        return Err("not gate must contain a matcher".to_string());
    }
    validate_regex_patterns(gate.regex, "not gate", "regex")?;
    validate_regex_patterns(gate.line_regex, "not gate", "line_regex")?;
    for nested in gate.all {
        validate_gate(nested.into(), "not all gate", depth + 1, complexity)?;
    }
    for nested in gate.any {
        validate_gate(nested.into(), "not any gate", depth + 1, complexity)?;
    }
    for nested in gate.not_gate {
        validate_not_gate(nested.into(), depth + 1, complexity)?;
    }
    Ok(())
}
/// Checks direct, aggregate and Unicode-scalar matcher limits.
fn validate_matcher_limits(
    gate: GateView<'_>,
    context: &str,
    complexity: &mut ManifestComplexity,
) -> Result<(), String> {
    let matcher_count = gate.contains.len() + gate.regex.len() + gate.line_regex.len();
    if matcher_count > MAX_MATCHERS_PER_GATE {
        return Err(format!(
            "{context} has {matcher_count} direct matchers, max is {MAX_MATCHERS_PER_GATE}"
        ));
    }
    complexity.total_matchers += matcher_count;
    if complexity.total_matchers > MAX_TOTAL_MATCHERS {
        return Err(format!(
            "manifest exceeds max matcher count {MAX_TOTAL_MATCHERS}"
        ));
    }
    for value in gate
        .contains
        .iter()
        .chain(gate.regex.iter())
        .chain(gate.line_regex.iter())
    {
        if value.chars().count() > MAX_MATCHER_CHARS {
            return Err(format!(
                "{context} matcher exceeds max length {MAX_MATCHER_CHARS}"
            ));
        }
    }
    Ok(())
}
/// Rejects malformed patterns before cache compilation.
fn validate_regex_patterns(patterns: &[String], context: &str, field: &str) -> Result<(), String> {
    for pattern in patterns {
        Regex::new(pattern).map_err(|err| {
            format!("{context} contains invalid {field} pattern {pattern:?}: {err}")
        })?;
    }
    Ok(())
}
/// Returns whether a gate has direct or nested positive evidence.
fn gate_has_positive_matcher(gate: GateView<'_>) -> bool {
    !gate.contains.is_empty()
        || !gate.regex.is_empty()
        || !gate.line_regex.is_empty()
        || !gate.all.is_empty()
        || !gate.any.is_empty()
}
/// Returns whether a gate has positive or exclusion evidence.
fn gate_has_any_matcher(gate: GateView<'_>) -> bool {
    gate_has_positive_matcher(gate) || !gate.not_gate.is_empty()
}
/// Validates fixed selectors and admitted parameterized line counts.
pub(super) fn validate_region_name(spec: &str) -> Result<(), String> {
    let trimmed = spec.trim();
    match trimmed {
        "whole_recent"
        | "after_last_prompt_marker"
        | "before_current_prompt_marker"
        | "whole_recent_without_current_prompt_marker"
        | "current_prompt_block_marker"
        | "after_current_prompt_block_marker"
        | "prompt_box_body"
        | "above_prompt_box"
        | "last_non_empty_above_prompt_box"
        | "after_last_horizontal_rule"
        | "osc_title"
        | "osc_progress" => Ok(()),
        _ if region_count(trimmed, "bottom_lines").is_some()
            || region_count(trimmed, "bottom_non_empty_lines").is_some()
            || top_region_count(trimmed).is_some() =>
        {
            Ok(())
        }
        _ => Err(trimmed.to_string()),
    }
}
/// Matches document identity or aliases through the catalogue parser.
pub(super) fn manifest_matches_agent(manifest: &AgentManifest, agent: Agent) -> bool {
    let id = agent_label(agent);
    manifest.id == id
        || manifest.aliases.iter().any(|alias| alias == id)
        || parse_agent_label(&manifest.id) == Some(agent)
        || manifest
            .aliases
            .iter()
            .any(|alias| parse_agent_label(alias) == Some(agent))
}
/// Borrows a rule's root matcher fields without copying its tree.
fn manifest_gate_from_rule(rule: &ManifestRule) -> GateView<'_> {
    GateView {
        all: &rule.all,
        any: &rule.any,
        not_gate: &rule.not_gate,
        contains: &rule.contains,
        regex: &rule.regex,
        line_regex: &rule.line_regex,
    }
}
/// Borrowed root or nested gate, avoiding copies during validation and compilation.
#[derive(Clone, Copy)]
struct GateView<'a> {
    /// Conjunctive descendants.
    all: &'a [ManifestGate],
    /// Alternative descendants.
    any: &'a [ManifestGate],
    /// Exclusion descendants.
    not_gate: &'a [ManifestGate],
    /// Authored case-insensitive needles.
    contains: &'a [String],
    /// Authored region patterns.
    regex: &'a [String],
    /// Authored line patterns.
    line_regex: &'a [String],
}
impl<'a> From<&'a ManifestGate> for GateView<'a> {
    fn from(gate: &'a ManifestGate) -> Self {
        Self {
            all: &gate.all,
            any: &gate.any,
            not_gate: &gate.not_gate,
            contains: &gate.contains,
            regex: &gate.regex,
            line_regex: &gate.line_regex,
        }
    }
}
/// Compiles every validated rule in document order.
pub(super) fn compile_manifest(manifest: &AgentManifest) -> Result<Vec<CompiledRule>, String> {
    manifest
        .rules
        .iter()
        .map(|rule| {
            compile_gate(manifest_gate_from_rule(rule))
                .map(|gate| CompiledRule { gate })
                .map_err(|err| format!("rule {} could not be compiled: {err}", rule.id))
        })
        .collect()
}
/// Compiles regexes and lowercases substring needles once per snapshot.
fn compile_gate(gate: GateView<'_>) -> Result<CompiledGate, String> {
    Ok(CompiledGate {
        all: gate
            .all
            .iter()
            .map(|gate| compile_gate(gate.into()))
            .collect::<Result<_, _>>()?,
        any: gate
            .any
            .iter()
            .map(|gate| compile_gate(gate.into()))
            .collect::<Result<_, _>>()?,
        not_gate: gate
            .not_gate
            .iter()
            .map(|gate| compile_gate(gate.into()))
            .collect::<Result<_, _>>()?,
        contains: gate
            .contains
            .iter()
            .map(|needle| needle.to_lowercase())
            .collect(),
        regex: gate
            .regex
            .iter()
            .map(|pattern| Regex::new(pattern).map_err(|err| err.to_string()))
            .collect::<Result<_, _>>()?,
        line_regex: gate
            .line_regex
            .iter()
            .map(|pattern| Regex::new(pattern).map_err(|err| err.to_string()))
            .collect::<Result<_, _>>()?,
    })
}

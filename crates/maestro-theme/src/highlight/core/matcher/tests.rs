//! Deterministic traversal witnesses through automatic language detection.
use super::SEARCH_BYTES;
use crate::highlight::{HighlightProfile, core};

/// Count searched spans, including exhausted suffixes, rather than wall time.
fn searched_bytes(code: &str) -> usize {
    SEARCH_BYTES.with(|work| work.set(0));
    core::detect(HighlightProfile::CodeBlock, code).unwrap();
    let work = SEARCH_BYTES.with(std::cell::Cell::get);
    println!("input bytes={}, searched span bytes={work}", code.len());
    work
}

#[test]
fn repeated_shell_lines_keep_search_work_linear() {
    let small = searched_bytes(&"echo \"$USER\"\n".repeat(64));
    let large = searched_bytes(&"echo \"$USER\"\n".repeat(128));
    assert!(
        large <= 2 * small + 128,
        "searched bytes: N={small}, 2N={large}"
    );
}

#[test]
fn unlabeled_python_integer_list_keeps_search_work_linear() {
    let small = searched_bytes(&format!("[{}]\n", "123, ".repeat(64)));
    let large = searched_bytes(&format!("[{}]\n", "123, ".repeat(128)));
    assert!(
        large <= 2 * small + 128,
        "searched bytes: N={small}, 2N={large}"
    );
}

#[test]
fn only_ignore_capable_begins_prepare_resumption_suffixes() {
    use crate::highlight::core::types::{Callback, RuleKind};
    core::tables(HighlightProfile::CodeBlock).unwrap();
    let tables = core::tables(HighlightProfile::ExportViewer).unwrap();
    assert_eq!(tables.modes.len(), 532);
    let mut prepared = 0;
    let ignores =
        |kind: &Callback| matches!(kind, Callback::NotDot | Callback::Shebang | Callback::Jsx);
    for (mode_index, mode) in tables.modes.iter().enumerate() {
        for (rule_index, rule) in mode.rules.iter().enumerate() {
            let expected = match rule.action {
                RuleKind::Begin(target) => {
                    let target = &tables.modes[target.0];
                    [target.before, target.on_begin].iter().any(ignores)
                        && rule_index + 1 < mode.begin_rule_count
                }
                RuleKind::End | RuleKind::Illegal => false,
            };
            assert_eq!(
                rule.resumption.get().is_some(),
                expected,
                "mode {mode_index}, rule {rule_index}"
            );
            prepared += usize::from(expected);
        }
    }
    assert_eq!(prepared, 63);
}

#[test]
fn builtin_capture_offsets_match_native_group_counts() {
    use super::super::Expression;
    use regress::Regex;
    let sources: Vec<Expression> = serde_json::from_str(include_str!(
        "../../../../assets/highlighting/patterns.json"
    ))
    .unwrap();
    let expected: Vec<_> = sources
        .iter()
        .map(|source| {
            Regex::with_flags(&format!("{}|", source.pattern), source.flags.as_str())
                .unwrap()
                .find("")
                .unwrap()
                .captures
                .len()
        })
        .collect();
    core::tables(HighlightProfile::CodeBlock).unwrap();
    for mode in &core::tables(HighlightProfile::ExportViewer).unwrap().modes {
        let matcher = mode.matcher.get().unwrap().as_ref().unwrap();
        for group in &matcher.groups {
            let pattern = mode.rules[group.rule_index].pattern.0;
            assert_eq!(
                group.count, expected[pattern],
                "{}",
                sources[pattern].pattern
            );
        }
    }
}

#[test]
fn profile_preparation_leaves_other_candidate_graph_uncompiled() {
    let tables = core::Tables::load(
        include_str!("../../../../assets/highlighting/patterns.json"),
        include_str!("../../../../assets/highlighting/keywords.json"),
        include_str!("../../../../assets/highlighting/modes.json"),
    )
    .unwrap();
    assert!(tables.modes.iter().all(|mode| mode.matcher.get().is_none()));
    tables.prepare(HighlightProfile::CodeBlock).unwrap();
    assert!(tables.modes[529].matcher.get().is_some());
    assert!(tables.modes[0].matcher.get().is_none());
    assert!(tables.modes[331].matcher.get().is_none());
    let shared_xml = tables.modes[445].matcher.get().unwrap();
    tables.prepare(HighlightProfile::ExportViewer).unwrap();
    assert!(tables.modes[0].matcher.get().is_some());
    assert!(tables.modes[331].matcher.get().is_some());
    assert!(std::ptr::eq(
        shared_xml,
        tables.modes[445].matcher.get().unwrap()
    ));
    assert_eq!(
        tables
            .patterns
            .iter()
            .filter(|pattern| pattern.get().is_some())
            .count(),
        149
    );
}

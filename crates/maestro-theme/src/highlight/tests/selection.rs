//! Ordered surface registries.
use crate::highlight::{HighlightProfile, languages};

#[test]
fn registered_profiles_keep_order_aliases_and_disabled_modes() {
    let viewer = languages::registered(HighlightProfile::ExportViewer);
    let browser = languages::registered(HighlightProfile::CodeBlock);
    assert_eq!(viewer.len(), 36);
    assert_eq!(viewer.iter().filter(|l| l.root.is_some()).count(), 34);
    assert_eq!(browser.len(), 9);
    assert_eq!((browser[3].name, browser[4].name), ("html", "xml"));
    assert_eq!(browser[3].root, browser[4].root);
    for profile in [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock] {
        for language in languages::registered(profile) {
            assert_eq!(
                languages::lookup(languages::registered(profile), language.name)
                    .unwrap()
                    .name,
                language.name
            );
        }
    }
    for name in ["c", "plaintext"] {
        assert!(
            languages::lookup(languages::registered(HighlightProfile::ExportViewer), name)
                .unwrap()
                .root
                .is_none()
        );
    }
}

/// Alias ownership recorded from each surface's independently loaded grammar set.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Alias {
    /// Explicit label accepted by the source registry.
    alias: String,
    /// Canonical owner after registration order is applied.
    canonical: String,
}

#[test]
fn profiles_keep_independently_recorded_alias_owners() {
    let aliases: super::fixtures::Profiles<Vec<Alias>> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/automatic_highlighting/detector-aliases.json"
    ))
    .unwrap();
    for profile in [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock] {
        for row in aliases.get(profile) {
            assert_eq!(
                languages::lookup(languages::registered(profile), &row.alias)
                    .map(|language| language.name),
                Some(row.canonical.as_str()),
                "{profile:?} {}",
                row.alias
            );
        }
    }
}

#[test]
fn detector_queries_match_complete_profile_corpus() {
    let corpus = super::fixtures::corpus();
    assert_eq!(corpus.inputs.len(), 105);
    let unique: std::collections::HashSet<_> = corpus.inputs.iter().map(|i| &i.code).collect();
    assert_eq!(unique.len(), corpus.inputs.len());
    for profile in [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock] {
        assert_eq!(
            languages::registered(profile)
                .iter()
                .map(|l| l.name)
                .collect::<Vec<_>>(),
            *corpus.registrations.get(profile)
        );
        for input in &corpus.inputs {
            super::fixtures::assert_result(profile, input);
        }
    }
}

#[test]
fn candidate_scores_match_profile_corpus() {
    use super::fixtures::{Candidates, corpus, engine, profile};
    use crate::highlight::core::IllegalPolicy;
    let corpus = corpus();
    let groups: Vec<Candidates> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/automatic_highlighting/detector-candidates.json"
    ))
    .unwrap();
    let mut observations = 0;
    for group in groups {
        let profile = profile(&group.profile);
        let engine = engine(profile);
        let input = corpus
            .inputs
            .iter()
            .find(|i| i.id == group.input_id)
            .unwrap();
        let languages: Vec<_> = engine
            .languages
            .iter()
            .filter(|l| l.root.is_some())
            .collect();
        assert_eq!(
            languages.iter().map(|l| l.name).collect::<Vec<_>>(),
            group
                .trials
                .iter()
                .map(|t| t.language.as_str())
                .collect::<Vec<_>>()
        );
        for (language, trial) in languages.iter().zip(group.trials) {
            let result = engine
                .evaluate(language, &input.code, None, IllegalPolicy::Reject)
                .score;
            assert_eq!(
                result.relevance().to_bits(),
                trial.relevance.to_bits(),
                "{} {}",
                input.id,
                language.name
            );
            assert_eq!(
                matches!(
                    result.outcome,
                    crate::highlight::core::types::Relevance::Illegal
                ),
                trial.illegal,
                "{} {}",
                input.id,
                language.name
            );
            observations += 1;
        }
    }
    assert_eq!(observations, 4515);
}

#[test]
fn profile_versions_keep_distinct_grammar_results() {
    super::fixtures::inputs(&[
        "trailing-empty",
        "json-trailing-comma",
        "json-comment",
        "html",
        "json-illegal",
        "wasm-module",
        "ruby-heredoc",
    ]);
}

#[test]
fn plain_candidate_and_equal_scores_keep_source_order() {
    use super::fixtures::{controlled, inputs};
    inputs(&["blank", "spaces"]);
    super::fixtures::witnesses(&["base", "superset"]);
    for profile in [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock] {
        let mut engine = controlled(profile);
        let mut reversed: Vec<_> = engine
            .languages
            .iter()
            .filter(|l| matches!(l.name, "base" | "superset"))
            .collect();
        reversed.reverse();
        // Move only these immutable descriptors into reversed registration order.
        let languages = [
            crate::highlight::core::types::Language {
                name: reversed[0].name,
                root: reversed[0].root,
                aliases: &[],
                superset: reversed[0].superset,
                fold: false,
            },
            crate::highlight::core::types::Language {
                name: reversed[1].name,
                root: reversed[1].root,
                aliases: &[],
                superset: reversed[1].superset,
                fold: false,
            },
        ];
        engine.languages = &languages;
        let rows = engine.automatic("same", &[]);
        assert_eq!(
            rows.iter().map(|r| r.language).collect::<Vec<_>>(),
            [Some("base"), Some("superset"), None]
        );
    }
}

#[test]
fn profile_lookup_keeps_unicode_case_and_raw_labels() {
    for profile in [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock] {
        assert_eq!(
            languages::lookup(languages::registered(profile), "JAVASCRIPT")
                .unwrap()
                .name,
            "javascript"
        );
        for raw in ["", "unknown", " javascript", "javascript extra"] {
            assert!(
                languages::lookup(languages::registered(profile), raw).is_none(),
                "{raw}"
            );
        }
    }
    assert_eq!(
        languages::lookup(
            languages::registered(HighlightProfile::ExportViewer),
            "\u{212a}T"
        )
        .unwrap()
        .name,
        "kotlin"
    );
    assert!(
        languages::lookup(
            languages::registered(HighlightProfile::CodeBlock),
            "\u{212a}T"
        )
        .is_none()
    );
    assert!(languages::lookup(languages::registered(HighlightProfile::CodeBlock), "c").is_none());
    assert_eq!(
        languages::lookup(
            languages::registered(HighlightProfile::ExportViewer),
            "toml"
        )
        .unwrap()
        .name,
        "ini"
    );
    assert!(
        languages::lookup(languages::registered(HighlightProfile::CodeBlock), "toml").is_none()
    );
    for (label, canonical) in [("zsh", "bash"), ("jsonc", "json")] {
        assert_eq!(
            languages::lookup(languages::registered(HighlightProfile::CodeBlock), label)
                .unwrap()
                .name,
            canonical
        );
        assert!(
            languages::lookup(languages::registered(HighlightProfile::ExportViewer), label)
                .is_none()
        );
    }
}

#[test]
fn repeated_and_reversed_queries_have_no_state_leak() {
    let corpus = super::fixtures::corpus();
    for profile in [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock] {
        for input in &corpus.inputs {
            super::fixtures::assert_result(profile, input);
        }
        for input in corpus.inputs.iter().rev() {
            super::fixtures::assert_result(profile, input);
        }
    }
}

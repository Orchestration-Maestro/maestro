//! Public frontmatter grammar and native field-selection witnesses.
#![cfg(test)]

use maestro_resources::{FrontmatterValue, parse_frontmatter, strip_frontmatter};
use serde::Deserialize;

/// Independent decoded oracle values, including signed zero and nonfinite numbers.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "lowercase",
    deny_unknown_fields
)]
enum Expected {
    Null,
    Boolean(bool),
    Number(String),
    String(String),
    Sequence(Vec<Self>),
    Mapping(Vec<(String, Self)>),
}

/// One complete authored document and its independently recorded result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    test: String,
    input: String,
    expected: Option<Expected>,
}

/// Compare all decoded members in order, without losing floating-point distinctions.
fn assert_value(actual: &FrontmatterValue, expected: &Expected) {
    match (actual, expected) {
        (FrontmatterValue::Null, Expected::Null) => (),
        (FrontmatterValue::Bool(a), Expected::Boolean(b)) => assert_eq!(a, b),
        (FrontmatterValue::String(a), Expected::String(b)) => assert_eq!(a, b),
        (FrontmatterValue::Number(a), Expected::Number(b)) => {
            let b: f64 = b.parse().unwrap();
            assert!(a.is_nan() && b.is_nan() || a.to_bits() == b.to_bits());
        }
        (FrontmatterValue::Sequence(a), Expected::Sequence(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_value(a, b);
            }
        }
        (FrontmatterValue::Mapping(a), Expected::Mapping(b)) => {
            assert_eq!(a.len(), b.len());
            for ((ak, av), (bk, bv)) in a.iter().zip(b) {
                assert_eq!(ak, bk);
                assert_value(av, bv);
            }
        }
        _ => panic!("decoded value kind differs"),
    }
}

/// Exercise the two public entry points after releasing the authored source.
fn check_family(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/frontmatter_grammar.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        count += 1;
        eprintln!("case {}", case.id);
        let parsed = parse_frontmatter(&case.input);
        let stripped = strip_frontmatter(&case.input);
        drop(case.input);
        let Some(expected) = case.expected else {
            assert!(parsed.is_err(), "{}", case.id);
            assert_eq!(stripped.unwrap_err(), parsed.unwrap_err(), "{}", case.id);
            continue;
        };
        let parsed = parsed.unwrap_or_else(|error| panic!("{}: {error}", case.id));
        let Expected::Mapping(fields) = expected else {
            panic!("expected document");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0, "frontmatter");
        assert_value(&parsed.frontmatter, &fields[0].1);
        assert_value(&FrontmatterValue::String(parsed.body.clone()), &fields[1].1);
        assert_eq!(stripped.unwrap(), parsed.body);
    }
    assert!(count > 0);
}

#[test]
fn frontmatter_framing_keeps_current_boundaries() {
    check_family("frontmatter_framing_keeps_current_boundaries");
}

#[test]
fn frontmatter_text_scalars_keep_comments_and_line_folding() {
    check_family("frontmatter_text_scalars_keep_comments_and_line_folding");
}

#[test]
fn frontmatter_block_collections_keep_nesting_and_empty_nodes() {
    check_family("frontmatter_block_collections_keep_nesting_and_empty_nodes");
}

#[test]
fn frontmatter_flow_collections_keep_delimiters_and_nulls() {
    check_family("frontmatter_flow_collections_keep_delimiters_and_nulls");
}

#[test]
fn frontmatter_block_scalars_keep_chomp_indent_and_folding() {
    check_family("frontmatter_block_scalars_keep_chomp_indent_and_folding");
}

#[test]
fn frontmatter_tags_keep_selected_scalar_kinds() {
    check_family("frontmatter_tags_keep_selected_scalar_kinds");
}

#[test]
fn frontmatter_anchors_keep_completed_values_and_latest_binding() {
    check_family("frontmatter_anchors_keep_completed_values_and_latest_binding");
}

#[test]
fn frontmatter_scalar_schema_keeps_core_spellings() {
    check_family("frontmatter_scalar_schema_keeps_core_spellings");
}

#[test]
fn frontmatter_rejects_malformed_grammar_before_field_selection() {
    check_family("frontmatter_rejects_malformed_grammar_before_field_selection");
}

#[test]
fn frontmatter_wide_mappings_keep_order_and_duplicate_identity() {
    check_family("frontmatter_wide_mappings_keep_order_and_duplicate_identity");
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
pub mod support;

/// Load isolated authored metadata through both native consumers.
#[cfg(not(target_arch = "wasm32"))]
fn native_load(
    yaml: &str,
) -> (
    maestro_resources::LoadSkillsResult,
    Vec<maestro_resources::PromptTemplate>,
) {
    use maestro_resources::{
        LoadPromptTemplatesOptions, LoadSkillsFromDirOptions, NativeResourceOperations,
        load_prompt_templates, load_skills_from_dir,
    };
    let dir = support::Directory::new();
    let input = format!("---\n{yaml}\n---\nBody");
    let skill_file = dir.file("sample/SKILL.md", &input);
    let prompt_file = dir.file("sample.md", &input);
    let skill = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            cwd: support::text(&dir.0),
            dir: support::text(skill_file.parent().unwrap()),
            source: "user",
        },
        &NativeResourceOperations,
    );
    let prompts = load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: support::text(&dir.0),
            home: support::text(&dir.0),
            agent_dir: support::text(&dir.0),
            config_dir_name: ".maestro",
            prompt_paths: &[support::text(&prompt_file).into()],
            include_defaults: false,
        },
        &NativeResourceOperations,
    )
    .unwrap();
    for item in &skill.skills {
        assert_eq!(item.file_path, skill_file);
        assert_eq!(item.source_info.path, skill_file);
        assert_eq!(item.source_info.scope, maestro_resources::SourceScope::User);
    }
    for diagnostic in &skill.diagnostics {
        assert_eq!(diagnostic.path.as_deref(), Some(support::text(&skill_file)));
    }
    for item in &prompts {
        assert_eq!(item.file_path, support::text(&prompt_file));
        assert_eq!(item.source_info.path, prompt_file);
        assert_eq!(item.content, "Body");
    }
    (skill, prompts)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn frontmatter_loaders_consume_owned_grammar_without_policy_changes() {
    for (yaml, description, hint) in [
        ("description: >-\n  first\n  second", "first second", None),
        ("base: &d authored\ndescription: *d", "authored", None),
        (
            "{description: authored, argument-hint: \"[x]\"}",
            "authored",
            Some("[x]"),
        ),
        (
            "description: authored\nextra: {a: [true, null, 2]}",
            "authored",
            None,
        ),
    ] {
        let (skills, prompts) = native_load(yaml);
        assert_eq!(skills.skills.len(), 1);
        assert!(skills.diagnostics.is_empty());
        assert_eq!(skills.skills[0].name, "sample");
        assert_eq!(skills.skills[0].description, description);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].description, description);
        assert_eq!(prompts[0].argument_hint.as_deref(), hint);
    }
    let (skills, prompts) = native_load("description: authored\nextra: [broken");
    assert!(skills.skills.is_empty());
    assert_eq!(skills.diagnostics.len(), 1);
    assert!(prompts.is_empty());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn frontmatter_loader_field_domains_keep_current_selection() {
    for (source, text) in [
        (None, None),
        (Some("null"), None),
        (Some("\"authored\""), Some("authored")),
        (Some("\"\""), Some("")),
    ] {
        for field in [
            "name",
            "description",
            "argument-hint",
            "disable-model-invocation",
        ] {
            string_domain(field, source, text);
        }
    }
    nonstring_domains();
}

/// Observe nullable/string fields through both native selection policies.
#[cfg(not(target_arch = "wasm32"))]
fn string_domain(field: &str, source: Option<&str>, text: Option<&str>) {
    let prefix = if field == "description" {
        ""
    } else {
        "description: selected\n"
    };
    let yaml = source.map_or_else(
        || prefix.into(),
        |source| format!("{prefix}{field}: {source}"),
    );
    let (skills, prompts) = native_load(&yaml);
    let valid_skill = field != "description" || text.is_some_and(|t| !t.is_empty());
    assert_eq!(skills.skills.len(), usize::from(valid_skill));
    assert_eq!(
        skills.diagnostics.len(),
        usize::from(!valid_skill || field == "name" && text == Some("authored"))
    );
    if valid_skill {
        let skill = &skills.skills[0];
        assert_eq!(
            skill.name,
            if field == "name" {
                text.filter(|t| !t.is_empty()).unwrap_or("sample")
            } else {
                "sample"
            }
        );
        assert_eq!(
            skill.description,
            if field == "description" {
                "authored"
            } else {
                "selected"
            }
        );
        assert!(!skill.disable_model_invocation);
    }
    assert_eq!(prompts.len(), 1);
    assert_eq!(
        prompts[0].description,
        if field == "description" {
            text.filter(|t| !t.is_empty()).unwrap_or("Body")
        } else {
            "selected"
        }
    );
    assert_eq!(
        prompts[0].argument_hint.as_deref(),
        if field == "argument-hint" {
            text.filter(|t| !t.is_empty())
        } else {
            None
        }
    );
}

/// Wrong-type fields retain native rejection and boolean-only invocation selection.
#[cfg(not(target_arch = "wasm32"))]
fn nonstring_domains() {
    for source in ["7", "false", "true", "{a: b}", "[a, b]"] {
        for field in [
            "name",
            "description",
            "argument-hint",
            "disable-model-invocation",
        ] {
            let yaml = if field == "description" {
                format!("description: {source}")
            } else {
                format!("description: selected\n{field}: {source}")
            };
            let (skills, prompts) = native_load(&yaml);
            let valid_skill = matches!(field, "argument-hint" | "disable-model-invocation");
            assert_eq!(skills.skills.len(), usize::from(valid_skill));
            assert_eq!(skills.diagnostics.len(), usize::from(!valid_skill));
            if valid_skill {
                assert_eq!(
                    skills.skills[0].disable_model_invocation,
                    field == "disable-model-invocation" && source == "true"
                );
            }
            assert_eq!(
                prompts.len(),
                usize::from(matches!(field, "name" | "disable-model-invocation"))
            );
        }
    }
}

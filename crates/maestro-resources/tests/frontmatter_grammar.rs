//! Public frontmatter grammar and native field-selection witnesses.

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

/// Complete decoded metadata and retained body.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedDocument {
    /// Independently decoded metadata.
    frontmatter: Expected,
    /// Independently retained body text.
    body: String,
}

/// One complete authored document and its independently recorded result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    test: String,
    input: String,
    expected: Option<ExpectedDocument>,
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
        assert_value(&parsed.frontmatter, &expected.frontmatter);
        assert_eq!(parsed.body, expected.body);
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
use crate::support;

/// Load isolated authored metadata through both native consumers.
#[cfg(not(target_arch = "wasm32"))]
fn native_load(
    yaml: &str,
) -> (
    maestro_resources::LoadSkillsResult,
    Vec<maestro_resources::PromptTemplate>,
) {
    use maestro_resources::{
        LoadPromptTemplatesOptions, LoadSkillsFromDirOptions, load_prompt_templates,
        load_skills_from_dir,
    };
    let dir = support::Directory::new();
    let input = format!("---\n{yaml}\n---\nBody");
    let skill_file = dir.file("sample/SKILL.md", &input);
    let prompt_file = dir.file("sample.md", &input);
    let skill = load_skills_from_dir(LoadSkillsFromDirOptions {
        cwd: support::text(dir.0.path()),
        dir: support::text(skill_file.parent().unwrap()),
        source: "user",
    });
    let prompts = load_prompt_templates(LoadPromptTemplatesOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: support::text(dir.0.path()),
        config_dir_name: ".maestro",
        prompt_paths: &[support::text(&prompt_file).into()],
        include_defaults: false,
    })
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
    assert_eq!(skills.diagnostics.len(), usize::from(!valid_skill));
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

/// Build a nested block collection through authored indentation.
fn nested_block(depth: usize, indicator: &str) -> String {
    let mut text = String::new();
    for indent in 0..depth {
        text.push_str(&" ".repeat(indent));
        text.push_str(indicator);
        text.push('\n');
    }
    text.push_str(&" ".repeat(depth));
    text.push('v');
    text
}

/// Observe each retained level rather than serializing through a depth-limited codec.
fn assert_nested_block(value: &FrontmatterValue, depth: usize, indicator: &str) {
    let mut current = value;
    for _ in 0..depth {
        current = match current {
            FrontmatterValue::Sequence(items) if indicator == "-" => {
                assert_eq!(items.len(), 1);
                &items[0]
            }
            FrontmatterValue::Mapping(items) if indicator == "k:" => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].0, "k");
                &items[0].1
            }
            _ => panic!("wrong nested collection shape"),
        };
    }
    assert_eq!(current, &FrontmatterValue::String("v".into()));
}

#[test]
fn frontmatter_depth_is_safe_on_a_worker_stack() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            for indicator in ["-", "k:"] {
                eprintln!("depth witness: {indicator} 800");
                let input = format!("---\n{}\n---\nBody", nested_block(800, indicator));
                let parsed = parse_frontmatter(&input).unwrap();
                assert_nested_block(&parsed.frontmatter, 800, indicator);
                assert_eq!(parsed.body, "Body");
                assert_eq!(strip_frontmatter(&input).unwrap(), "Body");
            }
            let yaml = format!("a: {}{}", "[".repeat(700), "]".repeat(700));
            let input = format!("---\n{yaml}\n---\nBody");
            eprintln!("depth witness: flow 700");
            let parsed = parse_frontmatter(&input).unwrap();
            let FrontmatterValue::Mapping(fields) = &parsed.frontmatter else {
                panic!("missing root mapping");
            };
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "a");
            let mut value = &fields[0].1;
            for _ in 1..700 {
                let FrontmatterValue::Sequence(items) = value else {
                    panic!("missing nested sequence");
                };
                assert_eq!(items.len(), 1);
                value = &items[0];
            }
            assert_eq!(value, &FrontmatterValue::Sequence(Vec::new()));
            assert_eq!(parsed.body, "Body");
            assert_eq!(strip_frontmatter(&input).unwrap(), "Body");
            assert_rejected_depths();
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Both public entry points reject excessive nesting without aborting.
fn assert_rejected_depths() {
    for depth in [1001, 100_000] {
        eprintln!("depth witness: flow {depth} rejection");
        let input = format!(
            "---\na: {}{}\n---\nBody",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        assert!(parse_frontmatter(&input).is_err());
        assert!(strip_frontmatter(&input).is_err());
    }
    for indicator in ["-", "k:"] {
        eprintln!("depth witness: {indicator} 1001 rejection");
        let input = format!("---\n{}\n---\nBody", nested_block(1001, indicator));
        assert!(parse_frontmatter(&input).is_err());
        assert!(strip_frontmatter(&input).is_err());
    }
}

#[test]
fn frontmatter_errors_retain_source_coordinates() {
    for (yaml, line, column) in [("key: \"ok\"junk", 1, 10), ("key: &a - item", 1, 9)] {
        let input = format!("---\n{yaml}\n---\nBody");
        let error = parse_frontmatter(&input).unwrap_err();
        assert_eq!(error.line, Some(line));
        assert_eq!(error.column, Some(column));
        assert_eq!(strip_frontmatter(&input).unwrap_err(), error);
    }
}

#[test]
fn frontmatter_implicit_key_limits_measure_text_units() {
    let mut failures = Vec::new();
    for width in [600, 1025] {
        let key = "名".repeat(width);
        for flow in [false, true] {
            let yaml = if flow {
                format!("[{key}: x]")
            } else {
                format!("{key}: x")
            };
            let input = format!("---\n{yaml}\n---\nBody");
            if width > 1024 {
                assert!(parse_frontmatter(&input).is_err());
                assert!(strip_frontmatter(&input).is_err());
                continue;
            }
            let parsed = match parse_frontmatter(&input) {
                Ok(parsed) => parsed,
                Err(error) => {
                    failures.push(format!("width {width}, flow {flow}: {error}"));
                    continue;
                }
            };
            let pair = FrontmatterValue::Mapping(vec![(
                key.clone(),
                FrontmatterValue::String("x".into()),
            )]);
            assert_eq!(
                parsed.frontmatter,
                if flow {
                    FrontmatterValue::Sequence(vec![pair])
                } else {
                    pair
                }
            );
            assert_eq!(parsed.body, "Body");
            assert_eq!(strip_frontmatter(&input).unwrap(), "Body");
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// The parser's result reaches both native resource consumers unchanged.
#[cfg(not(target_arch = "wasm32"))]
fn check_quoted_metadata(yaml: &str, expected: Option<&str>) {
    let input = format!("---\n{yaml}\n---\nBody");
    let parsed = maestro_resources::parse_frontmatter(&input);
    let (skills, templates) = native_load(yaml);
    if let Some(description) = expected {
        assert!(parsed.is_ok(), "{yaml}: {parsed:?}");
        assert!(
            skills.diagnostics.is_empty(),
            "{yaml}: {:?}",
            skills.diagnostics
        );
        assert_eq!(skills.skills.len(), 1, "{yaml}");
        assert_eq!(skills.skills[0].description, description, "{yaml}");
        assert_eq!(templates.len(), 1, "{yaml}");
        assert_eq!(templates[0].description, description, "{yaml}");
    } else {
        let error = parsed.unwrap_err();
        assert!(
            error.message.contains("Unclosed quoted scalar"),
            "{yaml}: {error}"
        );
        assert!(skills.skills.is_empty(), "{yaml}");
        assert_eq!(skills.diagnostics.len(), 1, "{yaml}");
        assert!(
            skills.diagnostics[0]
                .message
                .contains("Unclosed quoted scalar"),
            "{yaml}"
        );
        assert!(templates.is_empty(), "{yaml}");
    }
}

/// Escaped physical newlines follow the same block indentation rule as other lines.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn quoted_continuation_indentation_reaches_both_loaders() {
    check_quoted_metadata("description: \"first\\\nsecond\"", None);
    check_quoted_metadata("description: \"first\\\n  second\"", Some("firstsecond"));
    check_quoted_metadata("{description: \"first\\\nsecond\"}", Some("firstsecond"));
    check_quoted_metadata("{description: \"first\\\n  second\"}", Some("firstsecond"));
}

/// An escaped apostrophe cannot close a scalar, even when it is the last byte.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn quoted_termination_reaches_both_loaders() {
    check_quoted_metadata("description: 'done''", None);
    check_quoted_metadata("description: 'it''s'", Some("it's"));
    check_quoted_metadata("{description: 'done''}", None);
    check_quoted_metadata("{description: 'it''s'}", Some("it's"));
    check_quoted_metadata("description: \"done\\\"", None);
}

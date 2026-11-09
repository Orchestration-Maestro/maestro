//! Qualification of the shipped development prompt metadata.
#![cfg(not(target_arch = "wasm32"))]

use maestro_resources::{LoadPromptTemplatesOptions, load_prompt_templates};
use std::path::Path;

/// Shipped names, descriptions, hints and authored argument lines.
const EXPECTED: [(&str, &str, Option<&str>, Option<&str>); 4] = [
    ("cl", "Audit release metadata before a release", None, None),
    (
        "is",
        "Analyze bug reports and feature requests from GitHub issues",
        Some("<issue>"),
        Some("Analyze these GitHub issues: $ARGUMENTS"),
    ),
    (
        "pr",
        "Review pull requests by URL with linked-issue and code analysis",
        Some("<PR-URL>"),
        Some("Review the GitHub PRs at these URLs: $@"),
    ),
    (
        "wr",
        "Finish the current task through release-metadata audit, commit and push",
        Some("[instructions]"),
        Some("Extra instructions: $ARGUMENTS"),
    ),
];

#[test]
fn maestro_development_prompts_keep_argument_fields() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let user_root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("development-guidance-user");
    std::fs::create_dir_all(&user_root).unwrap();
    let mut templates = load_prompt_templates(LoadPromptTemplatesOptions {
        cwd: root.to_str().unwrap(),
        home: user_root.to_str().unwrap(),
        agent_dir: user_root.to_str().unwrap(),
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: true,
    })
    .unwrap();
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        templates
            .iter()
            .map(|template| template.name.as_str())
            .collect::<Vec<_>>(),
        ["cl", "is", "pr", "wr"]
    );
    for (template, (_, description, hint, argument)) in templates.iter().zip(EXPECTED) {
        assert_eq!(
            template.description, description,
            "{} description",
            template.name
        );
        assert_eq!(
            template.argument_hint.as_deref(),
            hint,
            "{} hint",
            template.name
        );
        if let Some(argument) = argument {
            assert!(
                template.content.lines().any(|line| line == argument),
                "{} literal arguments",
                template.name
            );
        }
    }
}

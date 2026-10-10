//! Qualification of the shipped development prompt metadata.
#![cfg(not(target_arch = "wasm32"))]

use maestro_resources::{
    LoadPromptTemplatesOptions, NativeResourceOperations, load_prompt_templates,
};
use std::path::Path;

/// Shipped names, descriptions, hints and authored argument lines.
const EXPECTED: [(&str, &str, Option<&str>, Option<&str>); 4] = [
    ("cl", "Audit changelog entries before release", None, None),
    (
        "is",
        "Analyze GitHub issues (bugs or feature requests)",
        Some("<issue>"),
        Some("Analyze GitHub issue(s): $ARGUMENTS"),
    ),
    (
        "pr",
        "Review PRs from URLs with structured issue and code analysis",
        Some("<PR-URL>"),
        Some("You are given one or more GitHub PR URLs: $@"),
    ),
    (
        "wr",
        "Finish the current task end-to-end with changelog, commit, and push",
        Some("[instructions]"),
        Some("Additional instructions: $ARGUMENTS"),
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
    let mut templates = load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: root.to_str().unwrap(),
            home: user_root.to_str().unwrap(),
            agent_dir: user_root.to_str().unwrap(),
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: true,
        },
        &NativeResourceOperations,
    )
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

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
    let paths: Vec<_> = EXPECTED
        .iter()
        .map(|(name, _, _, _)| {
            root.join(format!("docs/agents/prompts/{name}.md"))
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let templates = load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: root.to_str().unwrap(),
            home: root.to_str().unwrap(),
            agent_dir: root.to_str().unwrap(),
            config_dir_name: ".maestro",
            prompt_paths: &paths,
            include_defaults: false,
        },
        &NativeResourceOperations,
    )
    .unwrap();
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

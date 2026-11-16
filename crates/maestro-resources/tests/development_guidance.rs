//! Qualification of the shipped development prompt metadata.
#![cfg(not(target_arch = "wasm32"))]

use maestro_resources::{
    LoadPromptTemplatesOptions, expand_prompt_template, load_prompt_templates,
};
use std::path::Path;

/// Shipped names, argument hints and whether the body takes the typed arguments.
const EXPECTED: [(&str, Option<&str>, bool); 4] = [
    ("cl", None, false),
    ("is", Some("<issue>"), true),
    ("pr", Some("<PR-URL>"), true),
    ("wr", Some("[instructions]"), true),
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
    for (template, (name, hint, takes_arguments)) in templates.iter().zip(EXPECTED) {
        assert_eq!(template.argument_hint.as_deref(), hint, "{name} hint");
        let expanded = expand_prompt_template(
            &format!("/{name} first-sentinel second-sentinel"),
            std::slice::from_ref(template),
        );
        assert_eq!(
            expanded.contains("first-sentinel second-sentinel"),
            takes_arguments,
            "{name} receives the typed arguments"
        );
        assert!(
            !expanded.contains("$ARGUMENTS") && !expanded.contains("$@"),
            "{name} leaves no argument marker"
        );
    }
}

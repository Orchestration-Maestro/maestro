//! Native resource loading through shared escaped metadata.
#![cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
pub mod support;
use maestro_resources::{
    DiagnosticType, LoadPromptTemplatesOptions, LoadSkillsFromDirOptions, NativeResourceOperations,
    PromptTemplate, SourceOrigin, SourceScope, load_prompt_templates, load_skills_from_dir,
};
use support::Directory;

/// Paired description escapes retain the skill and its local provenance.
#[test]
fn skill_loader_decodes_paired_description() {
    let dir = Directory::new();
    let file = dir.file(
        "paired-skill/SKILL.md",
        "---\ndescription: \"\\uD83C\\uDF89\"\n---\nBody",
    );
    let base = support::text(file.parent().unwrap());
    let result = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            cwd: support::text(&dir.0),
            dir: base,
            source: "user",
        },
        &NativeResourceOperations,
    );
    assert_eq!(result.skills.len(), 1);
    assert!(result.diagnostics.is_empty());
    let skill = &result.skills[0];
    assert_eq!(skill.name, "paired-skill");
    assert_eq!(skill.description, "🎉");
    assert_eq!(skill.file_path, file);
    assert_eq!(skill.base_dir, base);
    assert_eq!(skill.source_info.path, file);
    assert_eq!(skill.source_info.base_dir.as_deref(), Some(base));
    assert_eq!(skill.source_info.source, "local");
    assert_eq!(skill.source_info.scope, SourceScope::User);
    assert_eq!(skill.source_info.origin, SourceOrigin::TopLevel);
    assert!(!skill.disable_model_invocation);
}

/// Load just the explicitly supplied native prompt file.
#[cfg(test)]
fn prompts(dir: &Directory, file: &std::path::Path) -> Vec<PromptTemplate> {
    load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: support::text(&dir.0),
            home: support::text(&dir.0),
            agent_dir: support::text(&dir.0),
            config_dir_name: ".maestro",
            prompt_paths: &[support::text(file).into()],
            include_defaults: false,
        },
        &NativeResourceOperations,
    )
    .unwrap()
}

/// A retained prompt exposes decoded metadata and its original body and identity.
#[test]
fn prompt_loader_decodes_paired_description_and_hint() {
    let dir = Directory::new();
    let file = dir.file(
        "paired-prompt.md",
        "---\ndescription: \"\\uD83C\\uDF89\"\nargument-hint: \"\\uD834\\uDD1E\"\n---\nBody",
    );
    let result = prompts(&dir, &file);
    assert_eq!(result.len(), 1);
    let prompt = &result[0];
    assert_eq!(prompt.name, "paired-prompt");
    assert_eq!(prompt.description, "🎉");
    assert_eq!(prompt.argument_hint.as_deref(), Some("𝄞"));
    assert_eq!(prompt.content, "Body");
    assert_eq!(prompt.file_path, support::text(&file));
    assert_eq!(prompt.source_info.path, file);
}

/// Invalid selected and unselected scalars produce one complete path-bearing warning.
#[test]
fn skill_loader_reports_invalid_surrogate_fields() {
    let dir = Directory::new();
    for (input, message) in [
        (
            "---\ndescription: \"\\uD800\"\n---\nBody",
            "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14",
        ),
        (
            "---\ndescription: \"\\uDC00\"\n---\nBody",
            "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14",
        ),
        (
            "---\ndescription: \"\\uDF89\\uD83C\"\n---\nBody",
            "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14",
        ),
        (
            "---\ndescription: \"\\uD83C \\uDF89\"\n---\nBody",
            "while parsing a quoted scalar, found invalid Unicode character escape code at byte 13 line 1 column 14",
        ),
        (
            "---\ndescription: ok\nextra: \"\\uD800\"\n---\nBody",
            "while parsing a quoted scalar, found invalid Unicode character escape code at byte 23 line 2 column 8",
        ),
    ] {
        let file = dir.file("invalid-skill/SKILL.md", input);
        let result = load_skills_from_dir(
            LoadSkillsFromDirOptions {
                cwd: support::text(&dir.0),
                dir: support::text(file.parent().unwrap()),
                source: "user",
            },
            &NativeResourceOperations,
        );
        assert!(result.skills.is_empty(), "{input}");
        assert_eq!(result.diagnostics.len(), 1, "{input}");
        let warning = &result.diagnostics[0];
        assert_eq!(warning.r#type, DiagnosticType::Warning);
        assert_eq!(warning.path.as_deref(), Some(support::text(&file)));
        assert_eq!(warning.message, message);
        assert!(warning.collision.is_none());
    }
}

/// Invalid selected and unselected scalars omit the prompt at the parse boundary.
#[test]
fn prompt_loader_skips_invalid_surrogate_fields() {
    let dir = Directory::new();
    for input in [
        "---\ndescription: \"\\uD800\"\n---\nBody",
        "---\ndescription: \"\\uDC00\"\n---\nBody",
        "---\ndescription: \"\\uDF89\\uD83C\"\n---\nBody",
        "---\ndescription: \"\\uD83C \\uDF89\"\n---\nBody",
        "---\ndescription: ok\nextra: \"\\uD800\"\n---\nBody",
    ] {
        let file = dir.file("invalid-prompt.md", input);
        assert!(prompts(&dir, &file).is_empty(), "{input}");
    }
}

/// Decoded field identity selects descriptions instead of unknown-key decoys.
#[test]
fn loaders_match_escaped_keys_without_sanitizing() {
    let dir = Directory::new();
    let input = "---\n\"descr\\u0069ption\": \"\\uD83C\\uDF89\"\n\"description\u{1f389}\": decoy\n\"\\uD83C\\uDF89\": ignored\n---\nBody";
    let skill_file = dir.file("escaped-key/SKILL.md", input);
    let skills = load_skills_from_dir(
        LoadSkillsFromDirOptions {
            cwd: support::text(&dir.0),
            dir: support::text(skill_file.parent().unwrap()),
            source: "user",
        },
        &NativeResourceOperations,
    );
    assert!(skills.diagnostics.is_empty());
    assert_eq!(skills.skills.len(), 1);
    assert_eq!(skills.skills[0].description, "🎉");
    let prompt_file = dir.file("escaped-key.md", input);
    let templates = prompts(&dir, &prompt_file);
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].description, "🎉");
}

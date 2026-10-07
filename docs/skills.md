# Skill resources

Load a supplied directory with `load_skills_from_dir`. Each retained `Skill` contains its name, description, file path, base directory, source information and model-invocation flag. Filesystem operations are supplied by the caller.

`load_skills` reads user skills before project skills, followed by explicit paths in input order. With defaults enabled, explicit paths have temporary scope. With defaults disabled, explicit paths beneath the supplied user or project skill directory retain that scope. Canonical file duplicates are omitted before name collisions; the first name wins.

Validation warnings do not prevent loading unless the description is missing or blank, or reading, parsing or a required string operation fails. Description warnings precede name warnings. Collision diagnostics follow all other diagnostics. An unignored file named `SKILL.md` ends discovery in its directory even when that file fails to load.

`parse_frontmatter` normalizes line endings and returns typed YAML metadata with the body. A leading three-dash opener and the first newline followed by three dashes delimit the metadata; only a delimited body is trimmed. Empty or null metadata becomes an empty object. `strip_frontmatter` returns the same body and propagates parsing errors.

`format_skills_for_prompt` renders visible skills in input order, using the skill file path as the location and XML-escaping each field. Only boolean `disable-model-invocation: true` hides a loaded skill. Native callers can supply `NativeResourceOperations`; portable callers provide `ResourceOperations`. Neither loading nor formatting executes a skill.

```rust
use maestro_resources::{
    LoadSkillsFromDirOptions, NativeResourceOperations,
    format_skills_for_prompt, load_skills_from_dir,
};

fn main() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!(
        "maestro-skill-example-{}", std::process::id()
    ));
    std::fs::create_dir(&dir)?;
    let file = dir.join("SKILL.md");
    std::fs::write(&file, "---\ndescription: Demonstration.\n---\n# Demo\n")?;
    let dir_text = dir.to_string_lossy();
    let result = load_skills_from_dir(
        LoadSkillsFromDirOptions { dir: &dir_text, source: "example" },
        &NativeResourceOperations,
    ).expect("directory loading succeeds");
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].source_info.source, "example");
    assert_eq!(result.skills[0].source_info.scope, "temporary");
    assert!(format_skills_for_prompt(&result.skills)
        .contains("<description>Demonstration.</description>"));
    std::fs::remove_file(file)?;
    std::fs::remove_dir(dir)?;
    Ok(())
}
```

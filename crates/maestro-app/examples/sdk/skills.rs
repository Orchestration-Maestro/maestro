//! Filter discovered skills and append an explicitly attributed SDK skill.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the skill-selection sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Load, filter and merge skills into the session prompt.
async fn run(sample: Sample) -> Result<Value, Error> {
    for name in ["browser", "unrelated-skill"] {
        let folder = sample.root.path().join("skills").join(name);
        std::fs::create_dir_all(&folder)?;
        std::fs::write(
            folder.join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: Instructions for {name}\n---\nRead carefully."
            ),
        )?;
    }
    let mut options = sample.options()?;
    if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
        resources.skills_override = Some(Box::new(merge_skills));
    }
    let created = maestro_app::create_agent_session(options).await?;
    let skills = created.session.resource_loader().get_skills();
    let result = json!({"skills": skills.skills, "prompt": created.session.system_prompt()});
    created.session.dispose();
    Ok(result)
}

/// Preserve selected discovered records and attach source metadata to the custom skill.
fn merge_skills(mut base: maestro_app::LoadSkillsResult) -> maestro_app::LoadSkillsResult {
    base.skills
        .retain(|skill| skill.name.contains("browser") || skill.name.contains("search"));
    base.skills.push(maestro_app::Skill {
        name: "my-skill".into(),
        description: "Custom project instructions".into(),
        file_path: "/virtual/SKILL.md".into(),
        base_dir: "/virtual".into(),
        source_info: maestro_resources::create_synthetic_source_info(
            "/virtual/SKILL.md".into(),
            maestro_resources::SyntheticSourceOptions {
                source: "sdk".into(),
                scope: None,
                origin: None,
                base_dir: None,
            },
        ),
        disable_model_invocation: false,
    });
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Filtering preserves discovered provenance while the custom skill retains its SDK source.
    #[test]
    fn skills_filters_discovery_and_preserves_custom_provenance() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["skills"][0]["name"], "browser");
        assert_eq!(value["skills"][1]["name"], "my-skill");
        assert_eq!(value["skills"][1]["sourceInfo"]["source"], "sdk");
        assert_eq!(value["skills"][1]["filePath"], "/virtual/SKILL.md");
        assert_eq!(value["skills"].as_array().ok_or("missing skills")?.len(), 2);
        let prompt = value["prompt"].as_str().ok_or("missing prompt")?;
        assert!(prompt.contains("my-skill"));
        assert!(prompt.contains("browser"));
        assert!(!prompt.contains("unrelated-skill"));
        Ok(())
    }
}

//! Filter discovered skills and append an explicitly attributed SDK skill.
mod support;
use std::io::Write;
use support::{Error, Sample};

/// Run the skill-selection sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Load, filter and merge skills into the session prompt.
async fn run(sample: Sample) -> Result<(), Error> {
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
    writeln!(
        std::io::stdout(),
        "Discovered skills: {}",
        skills
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    for diagnostic in &skills.diagnostics {
        writeln!(std::io::stdout(), "Warning: {}", diagnostic.message)?;
    }
    writeln!(std::io::stdout(), "Session created with filtered skills")?;
    created.session.dispose();
    Ok(())
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

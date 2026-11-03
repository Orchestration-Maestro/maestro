//! Default, empty and caller-supplied skill results through session creation.
use super::super::*;

/// Default discovery exposes the authored file on the session's resource owner.
#[test]
fn sessions_discover_skills_by_default() -> Result<(), Error> {
    support::execute(|sample| async move {
        let folder = sample.root.path().join("skills/test-skill");
        std::fs::create_dir_all(&folder)?;
        std::fs::write(
            folder.join("SKILL.md"),
            "---\nname: test-skill\ndescription: SDK instructions\n---\nRead carefully.",
        )?;
        let created = maestro_app::create_agent_session(sample.options()?).await?;
        let skills = created.session.resource_loader().get_skills();
        assert!(skills.skills.iter().any(|skill| skill.name == "test-skill"));
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// The SDK respects an explicitly empty loader result despite a discoverable file.
#[test]
fn sessions_keep_an_empty_supplied_skill_result() -> Result<(), Error> {
    support::execute(|sample| async move {
        let folder = sample.root.path().join("skills/test-skill");
        std::fs::create_dir_all(&folder)?;
        std::fs::write(
            folder.join("SKILL.md"),
            "---\nname: test-skill\ndescription: SDK instructions\n---\nRead carefully.",
        )?;
        let mut options = sample.options()?;
        if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
            resources.skills_override =
                Some(Box::new(|_| maestro_app::LoadSkillsResult::default()));
        }
        let created = maestro_app::create_agent_session(options).await?;
        let skills = created.session.resource_loader().get_skills();
        assert!(skills.skills.is_empty());
        assert!(skills.diagnostics.is_empty());
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// A loader-supplied skill remains intact instead of being replaced by default discovery.
#[test]
fn sessions_keep_caller_supplied_skills() -> Result<(), Error> {
    support::execute(|sample| async move {
        let mut options = sample.options()?;
        if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
            resources.skills_override = Some(Box::new(|_| {
                merge_skills(maestro_app::LoadSkillsResult::default())
            }));
        }
        let created = maestro_app::create_agent_session(options).await?;
        let skills = created.session.resource_loader().get_skills();
        assert_eq!(skills.skills.len(), 1);
        assert_eq!(skills.skills[0].name, "my-skill");
        assert_eq!(skills.skills[0].file_path, "/virtual/SKILL.md");
        assert_eq!(skills.skills[0].source_info.source, "sdk");
        assert!(skills.diagnostics.is_empty());
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

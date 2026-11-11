//! Session creation options reach the live session: tool selection, working directory,
//! prompt overrides, disabled resource kinds and caller-supplied skills and templates.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::no_commands::no_commands;
use crate::support::sdk_sample::{Error, Sample, execute, sdk_source, supplied_skill};
use maestro_app::{
    AuthStorage, CreateAgentSessionOptions, ModelRegistry, ResourceKind, SessionResources,
    create_agent_session,
};
use maestro_credentials::AuthStorageData;
use maestro_models::ModelThinkingLevel;
use serde_json::{Value, json};
use std::sync::Arc;

/// The default loader's options inside `options`.
fn resources(
    options: &mut CreateAgentSessionOptions,
) -> Result<&mut maestro_app::ResourceLoaderOptions<maestro_models::Local>, Error> {
    match &mut options.resources {
        SessionResources::Default(resources) => Ok(resources),
        SessionResources::Loader(_) => Err("the sample uses default resource discovery".into()),
    }
}

/// The session's active tool names and working directory for `tools` rooted at `cwd`.
async fn tool_selection(sample: &Sample, tools: &[&str], cwd: &str) -> Result<Value, Error> {
    let mut options = sample.options()?;
    options.cwd = Some(cwd.to_owned());
    options.tools = Some(tools.iter().map(|name| (*name).to_owned()).collect());
    let created = create_agent_session(options).await?;
    let context = created.session.extension_runner().create_context();
    let observed = json!({"tools": created.session.get_active_tool_names(), "cwd": context.cwd()?});
    created.session.dispose();
    Ok(observed)
}

/// An allowlist keeps its order for the default and for another working directory.
#[test]
fn tool_allowlists_keep_their_order_and_working_directory() -> Result<(), Error> {
    execute(|sample| async move {
        let root = sample.root.path().display().to_string();
        let project = maestro_path::join(&[&root, "project"]);
        std::fs::create_dir(&project)?;
        let read_only = tool_selection(&sample, &["read", "grep", "find", "ls"], &root).await?;
        assert_eq!(read_only["tools"], json!(["read", "grep", "find", "ls"]));
        assert_eq!(read_only["cwd"], root);
        let elsewhere =
            tool_selection(&sample, &["read", "bash", "edit", "write"], &project).await?;
        assert_eq!(elsewhere["tools"], json!(["read", "bash", "edit", "write"]));
        assert_eq!(elsewhere["cwd"], project);
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Replacement drops discovered append text, while appending keeps the default prompt
/// together with discovered and supplied instructions and context files.
#[test]
fn prompt_overrides_replace_or_append_and_context_files_merge() -> Result<(), Error> {
    execute(|sample| async move {
        std::fs::write(
            sample.root.path().join("APPEND_SYSTEM.md"),
            "Discovered append text",
        )?;
        std::fs::write(
            sample.root.path().join("AGENTS.md"),
            "Project rule: answer briefly.",
        )?;
        let mut replaced = sample.options()?;
        resources(&mut replaced)?.system_prompt_override =
            Some(Box::new(|_| Some("Speak like a pirate.".into())));
        resources(&mut replaced)?.append_system_prompt_override = Some(Box::new(|_| Vec::new()));
        let replaced = create_agent_session(replaced).await?;
        let prompt = replaced.session.system_prompt();
        assert!(prompt.starts_with("Speak like a pirate."));
        assert!(!prompt.contains("Discovered append text"));
        replaced.session.dispose();
        let mut appended = sample.options()?;
        resources(&mut appended)?.append_system_prompt_override = Some(Box::new(|mut base| {
            base.push("Always be concise".into());
            base
        }));
        resources(&mut appended)?.agents_files_override = Some(Box::new(|mut files| {
            files.push(maestro_models::system_prompt::ContextFile {
                path: "/virtual/AGENTS.md".into(),
                content: "Prefer immutable bindings.".into(),
            });
            files
        }));
        let appended = create_agent_session(appended).await?;
        let prompt = appended.session.system_prompt();
        for expected in [
            "Discovered append text",
            "Always be concise",
            "Project rule: answer briefly.",
            "Prefer immutable bindings.",
        ] {
            assert!(prompt.contains(expected), "{expected}");
        }
        let files = appended.session.resource_loader().get_agents_files();
        assert_eq!(files.len(), 2);
        assert_eq!(files[1].path, "/virtual/AGENTS.md");
        appended.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Disabled resource kinds, an explicit prompt, tools and thinking level are the whole configuration.
#[test]
fn disabled_resources_leave_only_the_supplied_configuration() -> Result<(), Error> {
    execute(|sample| async move {
        std::fs::write(
            sample.root.path().join("AGENTS.md"),
            "Unwanted disk instruction",
        )?;
        let mut options = sample.options()?;
        options.tools = Some(vec!["read".into(), "bash".into()]);
        options.thinking_level = Some(ModelThinkingLevel::Off);
        let loader = resources(&mut options)?;
        loader.disabled.extend([
            ResourceKind::Extensions,
            ResourceKind::Skills,
            ResourceKind::PromptTemplates,
            ResourceKind::Themes,
            ResourceKind::ContextFiles,
        ]);
        loader.system_prompt = Some("You are a minimal assistant.".into());
        loader.append_system_prompt = Some(Vec::new());
        let created = create_agent_session(options).await?;
        let discovered = created.session.resource_loader();
        assert_eq!(created.session.get_active_tool_names(), ["read", "bash"]);
        assert!(discovered.get_skills().skills.is_empty());
        assert!(discovered.get_prompts().prompts.is_empty());
        assert!(discovered.get_agents_files().is_empty());
        assert_eq!(json!(created.session.thinking_level()), "off");
        let prompt = created.session.system_prompt();
        assert!(prompt.starts_with("You are a minimal assistant."));
        assert!(!prompt.contains("Unwanted disk instruction"));
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Filtered discovery plus a supplied skill and template keep their provenance and reach the prompt.
#[test]
fn supplied_skills_and_templates_merge_with_discovered_ones() -> Result<(), Error> {
    execute(|sample| async move {
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
        let prompts = sample.root.path().join("prompts");
        std::fs::create_dir(&prompts)?;
        std::fs::write(prompts.join("review.md"), "Review the changes.")?;
        let mut options = sample.options()?;
        let loader = resources(&mut options)?;
        loader.skills_override = Some(Box::new(|mut base| {
            base.skills.retain(|skill| skill.name == "browser");
            base.skills.push(supplied_skill());
            base
        }));
        loader.prompts_override = Some(Box::new(|mut base| {
            base.prompts.push(supplied_template());
            base
        }));
        let created = create_agent_session(options).await?;
        let discovered = created.session.resource_loader();
        let skills = discovered.get_skills();
        let names: Vec<_> = skills
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect();
        assert_eq!(names, ["browser", "my-skill"]);
        assert_eq!(skills.skills[1].source_info.source, "sdk");
        let prompt = created.session.system_prompt();
        assert!(prompt.contains("my-skill") && prompt.contains("browser"));
        assert!(!prompt.contains("unrelated-skill"));
        let templates = created.session.prompt_templates();
        let names: Vec<_> = templates
            .prompts
            .iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(names, ["review", "deploy"]);
        assert_eq!(templates.prompts[1].source_info.source, "sdk");
        assert_eq!(
            maestro_resources::prompts::expand_prompt_template("/deploy", &templates.prompts),
            "# Deploy Instructions"
        );
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// A catalog read from another directory, and a built-in-only catalog, are the registries the
/// session reports for the models they describe.
#[test]
fn explicit_model_registries_are_the_ones_the_session_reports() -> Result<(), Error> {
    execute(|sample| async move {
        let custom_dir = sample.root.path().join("custom");
        std::fs::create_dir(&custom_dir)?;
        let mut catalog: Value =
            serde_json::from_slice(&std::fs::read(sample.root.path().join("models.json"))?)?;
        catalog["providers"]["faux"]["models"][0]["name"] = json!("Custom location model");
        let catalog_path = custom_dir.join("models.json");
        std::fs::write(&catalog_path, serde_json::to_vec(&catalog)?)?;
        let custom = Arc::new(ModelRegistry::create(
            Arc::new(AuthStorage::create(
                &custom_dir.join("auth.json").display().to_string(),
            )),
            catalog_path.display().to_string(),
            no_commands(),
        )?);
        let mut options = sample.options()?;
        options.auth_storage = Some(Arc::clone(custom.auth_storage()));
        options.model_registry = Some(Arc::clone(&custom));
        let created = create_agent_session(options).await?;
        assert!(Arc::ptr_eq(created.session.model_registry(), &custom));
        let model = custom
            .find("faux", "faux-1")
            .ok_or("missing custom model")?;
        assert_eq!(
            model.read().map_err(|error| error.to_string())?.name,
            "Custom location model"
        );
        created.session.dispose();
        let builtin = Arc::new(ModelRegistry::in_memory(
            Arc::new(AuthStorage::in_memory(AuthStorageData::default())),
            no_commands(),
        )?);
        let mut options = sample.options()?;
        options.model_registry = Some(Arc::clone(&builtin));
        let created = create_agent_session(options).await?;
        assert!(Arc::ptr_eq(created.session.model_registry(), &builtin));
        assert!(builtin.find("faux", "faux-1").is_none());
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// A template an SDK caller supplies without any file on disk.
fn supplied_template() -> maestro_app::PromptTemplate {
    maestro_app::PromptTemplate {
        name: "deploy".into(),
        description: "Deploy the application".into(),
        argument_hint: None,
        file_path: "/virtual/prompts/deploy.md".into(),
        source_info: sdk_source("/virtual/prompts/deploy.md"),
        content: "# Deploy Instructions".into(),
    }
}

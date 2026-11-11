//! Merge discovered prompt templates with a caller-authored deployment command.
mod support;
use std::io::Write;
use support::{Error, Sample};

/// Run the prompt-template sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Print the merged templates.
async fn run(sample: Sample) -> Result<(), Error> {
    let folder = sample.root.path().join("prompts");
    std::fs::create_dir(&folder)?;
    std::fs::write(folder.join("review.md"), "Review the changes.")?;
    let mut options = sample.options()?;
    if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
        resources.prompts_override = Some(Box::new(merge_templates));
    }
    let created = maestro_app::create_agent_session(options).await?;
    let templates = created.session.prompt_templates();
    writeln!(std::io::stdout(), "Discovered prompt templates:")?;
    for template in &templates.prompts {
        writeln!(
            std::io::stdout(),
            "  /{}: {}",
            template.name,
            template.description
        )?;
    }
    writeln!(
        std::io::stdout(),
        "Session created with {} prompt templates",
        templates.prompts.len()
    )?;
    created.session.dispose();
    Ok(())
}

/// Append a synthetic template without discarding discovered diagnostics.
fn merge_templates(mut base: maestro_app::LoadPromptsResult) -> maestro_app::LoadPromptsResult {
    base.prompts.push(maestro_app::PromptTemplate {
        name: "deploy".into(), description: "Deploy the application".into(), argument_hint: None,
        file_path: "/virtual/prompts/deploy.md".into(),
        source_info: maestro_resources::create_synthetic_source_info("/virtual/prompts/deploy.md".into(),
            maestro_resources::SyntheticSourceOptions { source: "sdk".into(), scope: None, origin: None, base_dir: None }),
        content: "# Deploy Instructions\n\n1. Build: npm run build\n2. Test: npm test\n3. Deploy: npm run deploy".into(),
    });
    base
}

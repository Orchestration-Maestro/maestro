//! Merge discovered prompt templates with a caller-authored deployment command.
mod support;
use serde_json::{Value, json};
use support::{Error, Sample};

/// Run the prompt-template sample.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Return merged templates and their expanded deployment command.
async fn run(sample: Sample) -> Result<Value, Error> {
    let folder = sample.root.path().join("prompts");
    std::fs::create_dir(&folder)?;
    std::fs::write(folder.join("review.md"), "Review the changes.")?;
    let mut options = sample.options()?;
    if let maestro_app::SessionResources::Default(resources) = &mut options.resources {
        resources.prompts_override = Some(Box::new(merge_templates));
    }
    let created = maestro_app::create_agent_session(options).await?;
    let templates = created.session.prompt_templates();
    let expanded =
        maestro_resources::prompt_templates::expand_prompt_template("/deploy", &templates.prompts);
    let records: Vec<Value> = templates
        .prompts
        .iter()
        .map(|template| {
            json!({
                "name": template.name, "sourceInfo": template.source_info,
                "filePath": template.file_path,
            })
        })
        .collect();
    let result = json!({"templates": records, "expanded": expanded});
    created.session.dispose();
    Ok(result)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Both templates survive loading and the SDK template expands with its source intact.
    #[test]
    fn prompt_templates_merges_and_expands_the_sdk_template() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["templates"][0]["name"], "review");
        assert_eq!(value["templates"][1]["name"], "deploy");
        assert_eq!(value["templates"][1]["sourceInfo"]["source"], "sdk");
        assert_eq!(
            value["expanded"],
            "# Deploy Instructions\n\n1. Build: npm run build\n2. Test: npm test\n3. Deploy: npm run deploy"
        );
        Ok(())
    }
}

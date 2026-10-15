//! Qualification through selected application resource roots.
#![cfg(test)]
#![cfg(unix)]
/// Shared recorded queries and assertions.
#[path = "support/default_configuration.rs"]
pub mod fixture;
use fixture::{Group, child};
use maestro_app::configuration::config::{CONFIG_DIR_NAME, paths};
use maestro_resources::prompt_templates::{LoadPromptTemplatesOptions, load_prompt_templates};
use maestro_resources::{NativeResourceOperations, SourceScope};
use serde::Deserialize;

/// Markdown discovery operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    text: String,
    name: String,
}
/// Selected prompt observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    description: String,
    content: String,
    argument_hint: Option<String>,
    scope: String,
}
/// Discover recorded Markdown through the application-selected root.
fn hint(group: Group) {
    for case in fixture::cases::<Input, Expected>(group) {
        let scratch = child::Scratch::new();
        let home = scratch.join("home");
        let root = paths::get_agent_dir(None, || Ok(home.clone())).unwrap();
        fixture::write(
            format!("{}/{}.md", paths::get_prompts_dir(&root), case.input.name),
            &case.input.text,
        );
        let prompts = load_prompt_templates(
            LoadPromptTemplatesOptions {
                cwd: scratch.0.to_str().unwrap(),
                home: &home,
                agent_dir: &root,
                config_dir_name: CONFIG_DIR_NAME,
                prompt_paths: &[],
                include_defaults: true,
            },
            &NativeResourceOperations,
        )
        .unwrap();
        assert_eq!(prompts.len(), 1);
        let prompt = &prompts[0];
        assert_eq!(prompt.name, case.input.name);
        assert_eq!(prompt.description, case.expected.description);
        assert_eq!(prompt.content, case.expected.content);
        assert_eq!(prompt.argument_hint, case.expected.argument_hint);
        assert_eq!(case.expected.scope, "user");
        assert_eq!(prompt.source_info.scope, SourceScope::User);
    }
}

#[test]
fn required_hint_survives_resolved_prompt_discovery() {
    hint(Group::RequiredHintSurvivesResolvedPromptDiscovery);
}

#[test]
fn special_hint_survives_resolved_prompt_discovery() {
    hint(Group::SpecialHintSurvivesResolvedPromptDiscovery);
}

#[test]
fn discovered_prompt_arguments_stay_literal() {
    literals();
}

/// Literal substitution operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiteralInput {
    /// Original template body.
    template: String,
    /// Parsed argument expectation.
    args: Vec<String>,
    /// Authored slash command.
    command: String,
}
/// Literal output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiteralExpected {
    /// Expanded body.
    expanded: String,
}
/// Exercise direct substitution and loaded-command expansion together.
fn literals() {
    use maestro_resources::prompt_templates::{
        expand_prompt_template, parse_command_args, substitute_args,
    };
    for case in
        fixture::cases::<LiteralInput, LiteralExpected>(Group::DiscoveredPromptArgumentsStayLiteral)
    {
        let scratch = child::Scratch::new();
        let root = paths::get_agent_dir(None, || Ok(scratch.join("home"))).unwrap();
        fixture::write(
            format!("{}/literal.md", paths::get_prompts_dir(&root)),
            &case.input.template,
        );
        let prompts = load_prompt_templates(
            LoadPromptTemplatesOptions {
                cwd: scratch.0.to_str().unwrap(),
                home: &scratch.join("home"),
                agent_dir: &root,
                config_dir_name: CONFIG_DIR_NAME,
                prompt_paths: &[],
                include_defaults: true,
            },
            &NativeResourceOperations,
        )
        .unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            parse_command_args(case.input.command.strip_prefix("/literal ").unwrap()),
            case.input.args
        );
        assert_eq!(
            substitute_args(&prompts[0].content, &case.input.args),
            case.expected.expanded
        );
        assert_eq!(
            expand_prompt_template(&case.input.command, &prompts),
            case.expected.expanded
        );
    }
}

#[test]
fn optional_hint_survives_resolved_prompt_discovery() {
    hint(Group::OptionalHintSurvivesResolvedPromptDiscovery);
}

#[test]
fn missing_hint_stays_absent_after_discovery() {
    hint(Group::MissingHintStaysAbsentAfterDiscovery);
}

#[test]
fn empty_hint_stays_absent_after_discovery() {
    hint(Group::EmptyHintStaysAbsentAfterDiscovery);
}

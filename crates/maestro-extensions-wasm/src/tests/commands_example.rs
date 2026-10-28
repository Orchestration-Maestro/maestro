//! Runs the shipped command selector through registered callbacks on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::example_support::{release_all, start};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/commands.rs"]
mod commands;

/// The factory the shipped component exports.
struct Commands;

impl Extension for Commands {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        commands::factory(api)
    }
}

/// Completion prefixes and their independent expected source labels.
fn completion_cases() -> Vec<(&'static str, Option<Vec<Value>>)> {
    [
        ("", Some(vec!["extension", "prompt", "skill"])),
        ("e", Some(vec!["extension"])),
        ("prompt", Some(vec!["prompt"])),
        ("s", Some(vec!["skill"])),
        ("extensions", None),
        ("E", None),
        (" skill", None),
    ]
    .into_iter()
    .map(|(prefix, labels)| {
        (
            prefix,
            labels.map(|labels| {
                labels
                    .into_iter()
                    .map(|label| json!({"value": label, "label": label, "description": null}))
                    .collect()
            }),
        )
    })
    .collect()
}

#[test]
fn commands_registers_metadata_and_completes_sources() -> Result<(), String> {
    let (host, exports) = super::block_on(start::<Commands>())?;
    assert_eq!(
        host.observed().registrations.commands,
        [(
            "commands".into(),
            Some("List available slash commands".into()),
            true
        )]
    );
    let completion = host.observed().callback("completion commands")?;
    for (prefix, expected) in completion_cases() {
        let items = super::block_on(exports.invoke_completion(completion, prefix.into()))?;
        assert_eq!(
            items.map(|items| items
                .into_iter()
                .map(|item| json!({
                    "value": item.value, "label": item.label, "description": item.description
                }))
                .collect::<Vec<_>>()),
            expected,
            "prefix {prefix:?}"
        );
    }
    release_all(&host, &["command commands", "completion commands"]);
    Ok(())
}

/// One command invocation with supplied host inventory and literal UI receipts.
struct Case {
    /// Argument text, including any surrounding whitespace.
    args: &'static str,
    /// Inventory returned by the host for this invocation.
    inventory: Value,
    /// Dialog responses in invocation order.
    answers: Value,
    /// Exact UI operations and operands the command must request.
    expected: Vec<(&'static str, Value)>,
}

/// Unsorted inventory with absent, empty and nonempty descriptions and paths.
fn inventory() -> Value {
    json!([
        {"name":"research", "description":"Find sources", "source":"skill",
         "sourceInfo":{"path":"./skills/research/SKILL.md", "source":"local", "scope":"project", "origin":"top-level"}},
        {"name":"greet", "description":"Hello 雪", "source":"extension",
         "sourceInfo":{"path":"../extensions/greet.rs", "source":"local", "scope":"user", "origin":"top-level"}},
        {"name":"draft", "description":"", "source":"prompt",
         "sourceInfo":{"path":"", "source":"local", "scope":"project", "origin":"top-level"}},
        {"name":"bare", "source":"extension",
         "sourceInfo":{"path":"/extensions/bare.rs", "source":"package", "scope":"user", "origin":"package"}}
    ])
}

/// The complete grouped selector, independent of the inventory's order.
fn selector() -> (&'static str, Value) {
    (
        "select",
        json!([
            "Available Commands",
            [
                "--- Extensions ---",
                "/greet - Hello 雪",
                "/bare",
                "--- Prompts ---",
                "/draft",
                "--- Skills ---",
                "/research - Find sources"
            ],
            null,
            null
        ]),
    )
}

/// Exact source filters retain the host's order within each group.
fn filtered_cases() -> Vec<Case> {
    [
        (
            " \t extension\n",
            inventory(),
            vec![(
                "select",
                json!([
                    "Available Commands",
                    ["--- Extensions ---", "/greet - Hello 雪", "/bare"],
                    null,
                    null
                ]),
            )],
        ),
        (
            "prompt",
            inventory(),
            vec![(
                "select",
                json!([
                    "Available Commands",
                    ["--- Prompts ---", "/draft"],
                    null,
                    null
                ]),
            )],
        ),
        (
            "skill",
            inventory(),
            vec![(
                "select",
                json!([
                    "Available Commands",
                    ["--- Skills ---", "/research - Find sources"],
                    null,
                    null
                ]),
            )],
        ),
    ]
    .into_iter()
    .map(|(args, inventory, expected)| Case {
        args,
        inventory,
        answers: json!([null]),
        expected,
    })
    .collect()
}

/// Empty matches, group headers and cancelled selection.
fn listing_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (args, supplied, expected) in [
        (
            "",
            json!([]),
            vec![("notify", json!(["No commands found", "Info"]))],
        ),
        ("", inventory(), vec![selector()]),
        (
            "extensions",
            inventory(),
            vec![("notify", json!(["No extensions commands found", "Info"]))],
        ),
        (
            "Extension",
            inventory(),
            vec![("notify", json!(["No Extension commands found", "Info"]))],
        ),
        (
            "skill",
            json!([]),
            vec![("notify", json!(["No skill commands found", "Info"]))],
        ),
    ] {
        cases.push(Case {
            args,
            inventory: supplied,
            answers: json!([null]),
            expected,
        });
    }
    cases.extend(filtered_cases());
    for answer in [
        json!("--- Extensions ---"),
        json!(""),
        json!("/missing"),
        json!("/draft"),
    ] {
        cases.push(Case {
            args: "",
            inventory: inventory(),
            answers: json!([answer]),
            expected: vec![selector()],
        });
    }
    cases
}

/// Supplies host decisions without implementing the example's selection policy.
fn supply(observed: &mut super::observed::Observed, case: &Case) {
    observed.actions.value = case.inventory.clone();
    observed
        .ui
        .control(&json!({"answers": case.answers}).to_string());
}

/// Checks all UI requests from this invocation, including their order and payloads.
fn assert_receipts(lines: &[String], case: &Case) {
    let actual: Vec<_> = super::ui_host::receipts(lines)
        .into_iter()
        .map(|receipt| (receipt.method, receipt.operands))
        .collect();
    let expected: Vec<_> = case
        .expected
        .iter()
        .map(|(method, operands)| ((*method).to_owned(), operands.clone()))
        .collect();
    assert_eq!(
        actual, expected,
        "arguments {:?}, replies {}",
        case.args, case.answers
    );
}

/// Invokes the retained command on the in-process adapter for each supplied case.
fn run_controlled(cases: &[Case]) -> Result<(), String> {
    let (host, exports) = super::block_on(start::<Commands>())?;
    let command = host.observed().callback("command commands")?;
    for case in cases {
        let mark = host.observed().transcript.len();
        supply(&mut host.observe(), case);
        super::block_on(exports.invoke_command(
            command,
            case.args.into(),
            host.command_context("/work"),
        ))?;
        assert_receipts(&host.observed().transcript[mark..], case);
    }
    release_all(&host, &["command commands", "completion commands"]);
    Ok(())
}

#[test]
fn commands_lists_filtered_inventory_and_handles_cancellation() -> Result<(), String> {
    run_controlled(&listing_cases())
}

/// Path notifications require affirmative confirmation.
fn path_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (selected, name, path) in [
        ("/greet - Hello 雪", "greet", "../extensions/greet.rs"),
        ("/bare", "bare", "/extensions/bare.rs"),
        (
            "/research - Find sources",
            "research",
            "./skills/research/SKILL.md",
        ),
    ] {
        for confirmed in [false, true] {
            let mut expected = vec![
                selector(),
                (
                    "confirm",
                    json!([name, format!("View source path?\n{path}"), null, null]),
                ),
            ];
            if confirmed {
                expected.push(("notify", json!([path, "Info"])));
            }
            cases.push(Case {
                args: "",
                inventory: inventory(),
                answers: json!([selected, confirmed]),
                expected,
            });
        }
    }
    cases
}

#[test]
fn commands_notifies_source_paths_only_after_confirmation() -> Result<(), String> {
    run_controlled(&path_cases())
}

/// Instantiates the shipped component and returns the host that serves its requests.
fn component() -> Result<super::host::Harness, String> {
    let path = super::build::example_component("commands")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    super::block_on(super::host::Harness::from_compiled(&engine, &component))
        .map_err(|error| error.to_string())
}

#[test]
fn commands_component_runs_registered_selection_and_completion() -> Result<(), String> {
    let mut harness = component()?;
    assert_eq!(
        harness.observed().registrations.commands,
        [(
            "commands".into(),
            Some("List available slash commands".into()),
            true
        )]
    );
    let command = harness
        .callback("command commands")
        .map_err(|error| error.to_string())?;
    for case in path_cases().into_iter().chain(listing_cases()) {
        let mark = harness.observed().transcript.len();
        supply(harness.observed(), &case);
        super::example_support::invoke_command_on_component(
            &mut harness,
            command,
            case.args,
            false,
        )?;
        assert_receipts(&harness.observed().transcript[mark..], &case);
    }
    let completion = harness
        .callback("completion commands")
        .map_err(|error| error.to_string())?;
    component_completions(&mut harness, completion)?;
    for key in [command, completion] {
        super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    }
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

/// Invokes the component's registered completion callback with every literal prefix.
fn component_completions(harness: &mut super::host::Harness, key: u32) -> Result<(), String> {
    for (prefix, expected) in completion_cases() {
        let exports = harness.exports.clone();
        let items = super::block_on(harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_completion(accessor, super::host::borrow(key), prefix.into())
                .await
        }))
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())??;
        let actual =
            items.map(|items| {
                items.into_iter().map(|item| json!({
            "value": item.value, "label": item.label, "description": item.description
        })).collect::<Vec<_>>()
            });
        assert_eq!(actual, expected, "prefix {prefix:?}");
    }
    Ok(())
}

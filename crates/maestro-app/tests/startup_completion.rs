//! Startup completion observed at application and native host boundaries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case<I, E> {
    input: I,
    expected: E,
}

fn cases<I: serde::de::DeserializeOwned, E: serde::de::DeserializeOwned>(
    name: &str,
) -> Vec<Case<I, E>> {
    let mut corpus: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/startup_completion.json")).unwrap();
    assert_eq!(corpus.len(), 28);
    serde_json::from_value(corpus.remove(name).unwrap()).unwrap()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct Command {
    name: String,
    description: String,
    #[serde(default)]
    argument_hint: Option<String>,
}

#[test]
fn ordinary_slash_commands_keep_every_name_description_and_order() {
    use maestro_app::presentation_data::slash_commands::BUILTIN_SLASH_COMMANDS;
    for case in cases::<Empty, Vec<Command>>(
        "ordinary_slash_commands_keep_every_name_description_and_order",
    ) {
        let Empty {} = case.input;
        let actual: Vec<_> = BUILTIN_SLASH_COMMANDS
            .iter()
            .map(|c| Command {
                name: c.name.clone(),
                description: c.description.clone(),
                argument_hint: c.argument_hint.clone(),
            })
            .collect();
        assert_eq!(actual, case.expected);
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MetadataInput {
    Label(LabelInput),
    Value(ValueInput),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelInput {
    source: String,
    description_present: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueInput {
    value: serde_json::Value,
}

#[test]
fn slash_command_records_keep_owned_source_metadata() {
    use maestro_app::{SlashCommandInfo, SlashCommandSource};
    use maestro_resources::source_info::{SourceInfo, SourceOrigin, SourceScope};
    assert_eq!(
        [
            SlashCommandSource::EXTENSION,
            SlashCommandSource::PROMPT,
            SlashCommandSource::SKILL
        ],
        ["extension", "prompt", "skill"]
    );
    for case in cases::<MetadataInput, serde_json::Value>(
        "slash_command_records_keep_owned_source_metadata",
    ) {
        let command: SlashCommandInfo = match case.input {
            MetadataInput::Label(input) => SlashCommandInfo {
                name: if input.source == "skill" {
                    "skill:example"
                } else {
                    "example"
                }
                .into(),
                description: input.description_present.then(String::new),
                source: SlashCommandSource(input.source),
                source_info: SourceInfo {
                    path: "commands/example".into(),
                    source: "package:fixture".into(),
                    scope: SourceScope::Project,
                    origin: SourceOrigin::Package,
                    base_dir: Some("commands".into()),
                },
            },
            MetadataInput::Value(input) => serde_json::from_value(input.value).unwrap(),
        };
        let resource: maestro_resources::SlashCommandInfo = command;
        assert_eq!(serde_json::to_value(resource).unwrap(), case.expected);
    }
}

#[cfg(unix)]
use crate::child;
#[cfg(unix)]
#[path = "support/startup_completion.rs"]
mod filesystem;

#[test]
#[cfg(unix)]
fn startup_without_legacy_inputs_is_inert() {
    filesystem::run("startup_without_legacy_inputs_is_inert");
}

#[test]
#[cfg(unix)]
fn persisted_aliases_are_rewritten_once() {
    filesystem::run("persisted_aliases_are_rewritten_once");
}

#[test]
#[cfg(unix)]
fn persisted_current_names_win_before_value_admission() {
    filesystem::run("persisted_current_names_win_before_value_admission");
}

#[test]
#[cfg(unix)]
fn persisted_nonobjects_invalid_and_current_files_keep_their_bytes() {
    filesystem::run("persisted_nonobjects_invalid_and_current_files_keep_their_bytes");
}

#[test]
#[cfg(unix)]
fn persisted_native_reader_failures_leave_the_document_unchanged() {
    filesystem::run("persisted_native_reader_failures_leave_the_document_unchanged");
}

#[test]
#[cfg(unix)]
fn persisted_rewrite_follows_links_and_tolerates_io_failures() {
    filesystem::run("persisted_rewrite_follows_links_and_tolerates_io_failures");
}

#[test]
#[cfg(unix)]
fn persisted_unknown_prototype_names_keep_literal_identity() {
    filesystem::run("persisted_unknown_prototype_names_keep_literal_identity");
}

#[test]
#[cfg(unix)]
fn managed_binary_names_move_in_fixed_order_once() {
    filesystem::run("managed_binary_names_move_in_fixed_order_once");
}

#[test]
#[cfg(unix)]
fn managed_binary_collisions_remove_only_old_files() {
    filesystem::run("managed_binary_collisions_remove_only_old_files");
}

#[test]
#[cfg(unix)]
fn managed_binary_source_directories_are_not_recursively_deleted() {
    filesystem::run("managed_binary_source_directories_are_not_recursively_deleted");
}

#[test]
#[cfg(unix)]
fn managed_binary_rename_and_delete_failures_are_tolerated() {
    filesystem::run("managed_binary_rename_and_delete_failures_are_tolerated");
}

#[test]
#[cfg(unix)]
fn managed_binary_create_failure_stops_later_conversions() {
    filesystem::run("managed_binary_create_failure_stops_later_conversions");
}

#[test]
#[cfg(unix)]
fn managed_binary_links_retain_their_original_referent() {
    filesystem::run("managed_binary_links_retain_their_original_referent");
}

#[test]
#[cfg(unix)]
fn managed_binary_links_do_not_switch_to_destination_decoys() {
    filesystem::run("managed_binary_links_do_not_switch_to_destination_decoys");
}

#[test]
#[cfg(unix)]
fn managed_binary_directory_links_keep_the_captured_directory() {
    filesystem::run("managed_binary_directory_links_keep_the_captured_directory");
}

#[test]
#[cfg(unix)]
fn managed_binary_sibling_and_intermediate_links_keep_identity() {
    filesystem::run("managed_binary_sibling_and_intermediate_links_keep_identity");
}

#[test]
#[cfg(unix)]
fn command_directories_convert_global_then_project_once() {
    filesystem::run("command_directories_convert_global_then_project_once");
}

#[test]
#[cfg(unix)]
fn command_rename_failures_warn_and_keep_going() {
    filesystem::run("command_rename_failures_warn_and_keep_going");
}

#[test]
#[cfg(unix)]
fn deprecated_directory_warning_domains_and_order_are_exact() {
    filesystem::run("deprecated_directory_warning_domains_and_order_are_exact");
}

#[test]
#[cfg(unix)]
fn startup_call_uses_supplied_cwd_and_call_time_root() {
    filesystem::run("startup_call_uses_supplied_cwd_and_call_time_root");
}

#[test]
#[cfg(unix)]
fn deprecated_directory_read_errors_are_tolerated() {
    filesystem::run("deprecated_directory_read_errors_are_tolerated");
}

#[test]
#[cfg(unix)]
fn startup_failure_keeps_later_conversions_unstarted() {
    filesystem::run("startup_failure_keeps_later_conversions_unstarted");
}

#[test]
#[cfg(unix)]
fn startup_order_observes_preceding_conversion_effects() {
    filesystem::run("startup_order_observes_preceding_conversion_effects");
}

#[test]
#[cfg(unix)]
fn persisted_rewrite_uses_shared_order_and_json_bytes() {
    filesystem::run_order("persisted_rewrite_uses_shared_order_and_json_bytes");
}

#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WarningInput {
    warnings: Vec<String>,
}
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WarningExpected {
    before: WarningPhase,
    after: WarningPhase,
}
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WarningPhase {
    output: String,
    done: bool,
}

#[cfg(unix)]
fn check_warning_pause(name: &str) {
    use maestro_app::configuration::migrations::show_deprecation_warnings;
    use std::{
        cell::{Cell, RefCell},
        future::{Future, poll_fn},
        task::{Context, Poll},
    };
    for case in cases::<WarningInput, WarningExpected>(name) {
        let ready = Cell::new(false);
        let polls = Cell::new(0);
        let output = RefCell::new(String::new());
        let wait = poll_fn(|_| {
            polls.set(polls.get() + 1);
            if ready.get() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        });
        let mut sink = |style, text: &str| {
            output
                .borrow_mut()
                .push_str(&filesystem::paint(style, text));
            Ok(())
        };
        let mut operation = std::pin::pin!(show_deprecation_warnings(
            &case.input.warnings,
            &mut sink,
            wait
        ));
        let mut context = Context::from_waker(futures_util::task::noop_waker_ref());
        let before = operation.as_mut().poll(&mut context);
        assert_eq!(before.is_ready(), case.expected.before.done);
        assert_eq!(*output.borrow(), case.expected.before.output);
        if case.input.warnings.is_empty() {
            assert_eq!(polls.get(), 0);
        } else {
            assert_eq!(polls.get(), 1);
            ready.set(true);
            let after = operation.as_mut().poll(&mut context);
            assert_eq!(after.is_ready(), case.expected.after.done);
            assert!(matches!(after, Poll::Ready(Ok(()))));
        }
        assert_eq!(*output.borrow(), case.expected.after.output);
    }
}
#[test]
#[cfg(unix)]
fn empty_deprecation_output_does_not_acquire_input() {
    check_warning_pause("empty_deprecation_output_does_not_acquire_input");
}
#[test]
#[cfg(unix)]
fn deprecation_output_waits_for_one_host_completion() {
    check_warning_pause("deprecation_output_waits_for_one_host_completion");
}

#[test]
fn migration_output_and_wait_errors_preserve_their_phase() {
    use futures_util::FutureExt;
    use maestro_app::configuration::migrations::{MigrationStyle, show_deprecation_warnings};
    use std::{
        cell::{Cell, RefCell},
        future::poll_fn,
        io,
        task::Poll,
    };
    for failure in ["output", "wait", "blank"] {
        let polled = Cell::new(false);
        let lines = RefCell::new(Vec::new());
        let host = poll_fn(|_| {
            polled.set(true);
            Poll::Ready(if failure == "wait" {
                Err(io::Error::from_raw_os_error(123))
            } else {
                Ok(())
            })
        });
        let mut sink = |style, text: &str| {
            if failure == "output" || (failure == "blank" && style == MigrationStyle::Plain) {
                return Err(io::Error::from_raw_os_error(123));
            }
            lines.borrow_mut().push((style, text.to_owned()));
            Ok(())
        };
        let error = show_deprecation_warnings(&["notice".into()], &mut sink, host)
            .now_or_never()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(123));
        assert_eq!(polled.get(), failure != "output");
        let expected = if failure == "output" {
            Vec::new()
        } else {
            vec![
            (MigrationStyle::Warning,"Warning: notice".into()),
            (MigrationStyle::Warning,"\nMove your extensions to the extensions/ directory.".into()),
            (MigrationStyle::Warning,"Migration guide: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/application/startup.md#native-legacy-conversions".into()),
            (MigrationStyle::Warning,"Documentation: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md".into()),
            (MigrationStyle::Dim,"\nPress any key to continue...".into()),
        ]
        };
        assert_eq!(*lines.borrow(), expected);
    }
}

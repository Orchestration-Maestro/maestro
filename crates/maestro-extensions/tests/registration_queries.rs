//! Owned registration and current query behavior.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/registration.rs"]
mod support;
#[test]
fn owned_registration_records_keep_sources_and_callbacks() {
    support::run("owned_registration_records_keep_sources_and_callbacks");
}
#[test]
fn same_owner_command_replacement_keeps_position() {
    support::run("same_owner_command_replacement_keeps_position");
}
#[test]
fn reserved_shortcuts_block_effective_keys() {
    support::run("reserved_shortcuts_block_effective_keys");
}

#[test]
fn same_owner_shortcut_replacement_keeps_position() {
    support::run("same_owner_shortcut_replacement_keeps_position");
}

#[test]
fn same_owner_flag_replacement_keeps_position() {
    support::run("same_owner_flag_replacement_keeps_position");
}

#[test]
fn command_queries_preserve_load_order_and_refresh() {
    support::run("command_queries_preserve_load_order_and_refresh");
}

#[test]
fn duplicate_commands_receive_distinct_invocations() {
    support::run("duplicate_commands_receive_distinct_invocations");
}

#[test]
fn literal_suffix_names_avoid_taken_invocations() {
    support::run("literal_suffix_names_avoid_taken_invocations");
}

#[test]
fn command_lookup_uses_resolved_names() {
    support::run("command_lookup_uses_resolved_names");
}

#[test]
fn command_completions_keep_none_empty_and_values() {
    support::run("command_completions_keep_none_empty_and_values");
}

#[test]
fn command_and_shortcut_callbacks_keep_context_and_errors() {
    support::run("command_and_shortcut_callbacks_keep_context_and_errors");
}

#[test]
fn flag_defaults_seed_only_missing_values() {
    support::run("flag_defaults_seed_only_missing_values");
}

#[test]
fn unregistered_flags_do_not_leak_shared_values() {
    support::run("unregistered_flags_do_not_leak_shared_values");
}

#[test]
fn flags_keep_first_owner_but_all_owner_defaults_are_shared() {
    support::run("flags_keep_first_owner_but_all_owner_defaults_are_shared");
}

#[test]
fn flag_value_snapshots_are_independent_and_ordered() {
    support::run("flag_value_snapshots_are_independent_and_ordered");
}

#[test]
fn registration_guard_preserves_first_stale_message() {
    support::run("registration_guard_preserves_first_stale_message");
}

#[test]
fn stale_registration_changes_nothing_and_queries_remain_readable() {
    support::run("stale_registration_changes_nothing_and_queries_remain_readable");
}

#[test]
fn rebound_reserved_keys_release_old_bindings() {
    support::run("rebound_reserved_keys_release_old_bindings");
}

#[test]
fn reserved_and_nonreserved_overlap_cannot_unlock_keys() {
    support::run("reserved_and_nonreserved_overlap_cannot_unlock_keys");
}

#[test]
fn reserved_multikey_bindings_block_every_key() {
    support::run("reserved_multikey_bindings_block_every_key");
}

#[test]
fn nonreserved_bindings_warn_and_allow_overrides() {
    support::run("nonreserved_bindings_warn_and_allow_overrides");
}

#[test]
fn later_builtin_claims_determine_nonreserved_diagnostic() {
    support::run("later_builtin_claims_determine_nonreserved_diagnostic");
}

#[test]
fn shortcut_normalization_preserves_raw_registration_text() {
    support::run("shortcut_normalization_preserves_raw_registration_text");
}

#[test]
fn shortcut_duplicates_keep_position_and_last_owner() {
    support::run("shortcut_duplicates_keep_position_and_last_owner");
}

#[test]
fn shortcut_diagnostics_reset_without_mutating_old_result() {
    support::run("shortcut_diagnostics_reset_without_mutating_old_result");
}

#[test]
fn installed_ui_suppresses_only_warning_output() {
    support::run("installed_ui_suppresses_only_warning_output");
}

#[test]
fn empty_and_unset_bindings_add_no_conflicts() {
    support::run("empty_and_unset_bindings_add_no_conflicts");
}

#[test]
fn shortcut_warning_text_preserves_paths_and_keys() {
    support::run("shortcut_warning_text_preserves_paths_and_keys");
}

#[test]
fn owner_collection_edits_are_observed_without_rebuilding_runner() {
    support::run("owner_collection_edits_are_observed_without_rebuilding_runner");
}

#[test]
fn retained_flags_and_command_sources_follow_their_owners() {
    support::run("retained_flags_and_command_sources_follow_their_owners");
}

#[test]
fn callback_lifetimes_end_after_last_retained_owner() {
    use maestro_extensions::types::{ExtensionRuntimeState, RegisteredCommandOptions};
    use maestro_extensions::{
        Extension, ExtensionAPI, ExtensionRunner, SourceInfo, create_event_bus,
    };
    use std::sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    };
    struct Capture(Arc<AtomicBool>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let released = Arc::new(AtomicBool::new(false));
    let capture = Capture(Arc::clone(&released));
    let source: SourceInfo = serde_json::from_value(serde_json::json!({"path":"/owned", "source":"cli", "scope":"temporary", "origin":"top-level"})).unwrap();
    let owner = Arc::new(RwLock::new(Extension::<String>::new(
        "/owned".into(),
        "/resolved".into(),
        Arc::new(RwLock::new(source)),
    )));
    let owners = Arc::new(RwLock::new(vec![Arc::clone(&owner)]));
    let runtime = Arc::new(ExtensionRuntimeState::new());
    let bus = create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
    let api = ExtensionAPI::new(&owner, Arc::clone(&runtime), bus.for_extension());
    api.register_command(
        "hello".into(),
        RegisteredCommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Arc::new(move |args, ctx: String| {
                assert!(!capture.0.load(Ordering::SeqCst));
                assert_eq!(args, "raw args");
                assert_eq!(ctx, "owned context");
                Box::pin(std::future::ready(Ok(())))
            }),
        },
    )
    .unwrap();
    let runner = ExtensionRunner::<String>::new(
        Arc::clone(&owners),
        runtime,
        support::session::runner_session("/workspace").0,
        Arc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    let command = runner.get_command("hello").unwrap();
    owners.write().unwrap().clear();
    drop((api, owner, runner, owners, bus));
    let executor = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    executor
        .block_on((command.handler)("raw args".into(), "owned context".into()))
        .unwrap();
    assert!(!released.load(Ordering::SeqCst));
    drop(command);
    assert!(released.load(Ordering::SeqCst));
}

#[cfg(test)]
mod tests {
    #[test]
    fn captured_command_facade_does_not_retain_owner() {
        run_captured_facade("command");
    }

    #[test]
    fn captured_completion_facade_does_not_retain_owner() {
        run_captured_facade("completion");
    }

    #[test]
    fn captured_shortcut_facade_does_not_retain_owner() {
        run_captured_facade("shortcut");
    }

    fn run_captured_facade(kind: &str) {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(captured_facade_releases_owner(kind));
    }

    use maestro_extensions::types::{
        ExtensionFlagOptions, ExtensionHandle, ExtensionRuntimeState, ExtensionShortcutOptions,
        FlagType, FlagValue, RegisteredCommandOptions,
    };
    use maestro_extensions::{
        Extension, ExtensionAPI, ExtensionContext, ExtensionRunner, SourceInfo, create_event_bus,
    };
    use std::sync::{Arc, RwLock};

    fn live_facade() -> (ExtensionHandle<()>, ExtensionAPI<()>) {
        let source: SourceInfo = serde_json::from_value(serde_json::json!({
            "path":"/captured", "source":"cli", "scope":"temporary", "origin":"top-level"
        }))
        .unwrap();
        let owner = Arc::new(RwLock::new(Extension::new(
            "/captured".into(),
            "/resolved".into(),
            Arc::new(RwLock::new(source)),
        )));
        let bus = create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
        let api = ExtensionAPI::new(
            &owner,
            Arc::new(ExtensionRuntimeState::new()),
            bus.for_extension(),
        );
        api.register_flag(
            "live".into(),
            ExtensionFlagOptions {
                description: None,
                r#type: FlagType::Boolean,
                default: Some(FlagValue::Boolean(true)),
            },
        )
        .unwrap();
        (owner, api)
    }

    async fn captured_facade_releases_owner(kind: &str) {
        let (owner, api) = live_facade();
        let weak = Arc::downgrade(&owner);
        let captured = api.clone();
        let read_flag = move || {
            assert_eq!(
                captured.get_flag("live").unwrap(),
                Some(FlagValue::Boolean(true))
            );
            captured
                .register_flag(
                    "nested".into(),
                    ExtensionFlagOptions {
                        description: None,
                        r#type: FlagType::String,
                        default: None,
                    },
                )
                .unwrap();
        };
        install_and_invoke(kind, &owner, &api, read_flag).await;
        assert!(owner.read().unwrap().flags.contains_key("nested"));
        drop(owner);
        assert!(
            weak.upgrade().is_none(),
            "{kind} captured facade leaked its owner"
        );
        assert!(
            api.get_flag("live")
                .unwrap_err()
                .to_string()
                .starts_with("This extension ctx is stale")
        );
        assert!(
            api.register_flag(
                "late".into(),
                ExtensionFlagOptions {
                    description: None,
                    r#type: FlagType::Boolean,
                    default: None,
                }
            )
            .unwrap_err()
            .to_string()
            .starts_with("This extension ctx is stale")
        );
    }

    /// A context from a runner without registrations.
    fn context() -> ExtensionContext {
        ExtensionRunner::<()>::new(
            Arc::new(RwLock::new(Vec::new())),
            Arc::new(ExtensionRuntimeState::new()),
            crate::support::session::runner_session("/workspace").0,
            Arc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        )
        .create_context()
    }

    async fn install_and_invoke(
        kind: &str,
        owner: &ExtensionHandle<()>,
        api: &ExtensionAPI<()>,
        read_flag: impl Fn() + Send + Sync + 'static,
    ) {
        if kind == "shortcut" {
            api.register_shortcut(
                "ctrl+y".into(),
                ExtensionShortcutOptions {
                    description: None,
                    handler: Arc::new(move |_: ExtensionContext| {
                        read_flag();
                        Box::pin(std::future::ready(Ok(())))
                    }),
                },
            )
            .unwrap();
            let handler = Arc::clone(
                &owner.read().unwrap().shortcuts["ctrl+y"]
                    .read()
                    .unwrap()
                    .handler,
            );
            handler(context()).await.unwrap();
            return;
        }
        let options: RegisteredCommandOptions<()> = if kind == "command" {
            RegisteredCommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Arc::new(move |_, ()| {
                    read_flag();
                    Box::pin(std::future::ready(Ok(())))
                }),
            }
        } else {
            RegisteredCommandOptions {
                description: None,
                get_argument_completions: Some(Arc::new(move |_| {
                    read_flag();
                    Box::pin(std::future::ready(Ok(None)))
                })),
                handler: Arc::new(|_, ()| Box::pin(std::future::ready(Ok(())))),
            }
        };
        api.register_command("captured".into(), options).unwrap();
        let command = Arc::clone(&owner.read().unwrap().commands["captured"]);
        if kind == "command" {
            let handler = Arc::clone(&command.read().unwrap().handler);
            handler(String::new(), ()).await.unwrap();
        } else {
            let complete = command
                .read()
                .unwrap()
                .get_argument_completions
                .clone()
                .unwrap();
            assert!(complete(String::new()).await.unwrap().is_none());
        }
    }
}

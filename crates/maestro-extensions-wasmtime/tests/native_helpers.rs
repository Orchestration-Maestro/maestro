//! Compiled author observations through the production loader and native helper owners.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, create_extension_runtime, load_extension_from_factory,
};
use maestro_extensions_wasmtime::artifact_factory;
use serde_json::Value;
use std::{rc::Rc, sync::RwLock};

#[test]
fn helper_width_keeps_unicode_and_ansi() {
    check("helper_width_keeps_unicode_and_ansi");
}

#[test]
fn helper_terminal_normalization_uses_native_owner() {
    check("helper_terminal_normalization_uses_native_owner");
}

#[test]
fn helper_ansi_offsets_follow_utf8_boundaries() {
    check("helper_ansi_offsets_follow_utf8_boundaries");
}

#[test]
fn helper_character_classes_keep_boundaries() {
    check("helper_character_classes_keep_boundaries");
}

#[test]
fn helper_compiled_guest_calls_production_owners() {
    check("helper_compiled_guest_calls_production_owners");
}

#[test]
fn helper_wrap_preserves_styled_spaces() {
    check("helper_wrap_preserves_styled_spaces");
}

#[test]
fn helper_background_callback_receives_padded_line_once() {
    check("helper_background_callback_receives_padded_line_once");
}

#[test]
fn helper_width_truncation_keeps_options() {
    check("helper_width_truncation_keeps_options");
}

#[test]
fn helper_column_slice_keeps_strict_and_width() {
    check("helper_column_slice_keeps_strict_and_width");
}

#[test]
fn helper_segments_keep_both_widths_and_style() {
    check("helper_segments_keep_both_widths_and_style");
}

/// Executes every recorded row of one behavior through the unchanged author fixture.
fn check(name: &str) {
    super::components::local(async {
        let bus = super::components::TracedBus::new();
        let runtime = create_extension_runtime::<Local>();
        let path = std::env::var("MAESTRO_HELPER_COMPONENT").unwrap();
        let owner = load_extension_from_factory(
            artifact_factory(path.clone()),
            "/helpers".into(),
            bus.controller.for_extension(),
            runtime.clone(),
            Some(path),
        )
        .await
        .unwrap();
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner.clone()])),
            runtime,
            super::session::runner_session("/helpers").0,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        let handler = runner.get_command("helpers").unwrap().command.handler;
        let rows: Vec<Value> =
            serde_json::from_str(include_str!("fixtures/helper_corpus.json")).unwrap();
        let mut count = 0;
        for row in rows.iter().filter(|row| row["test"] == name) {
            let result = handler(row.to_string(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string();
            let observed: Value =
                serde_json::from_str(&result).unwrap_or_else(|e| panic!("{e}: {result}"));
            assert_eq!(observed, row["ok"], "{row}");
            count += 1;
        }
        assert_ne!(count, 0);
        drop(owner);
        drop(runner);
        bus.controller.clear();
    });
}

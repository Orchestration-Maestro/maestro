//! Direct shell rows through their retained public component.
#![cfg(test)]
use crate::manual_runtime;
use crate::published;
use crate::recording_terminal;
use maestro_chat::BashExecutionComponent;
use maestro_tui::{Component, TUI, TerminalImage};
use std::rc::Rc;
#[path = "shell_rows_support/mod.rs"]
mod shell_rows_support;
#[test]
fn shell_initial_frame_retains_command_and_cancel_keys() {
    shell_rows_support::replay("shell_initial_frame_retains_command_and_cancel_keys");
}
#[test]
fn shell_output_chunks_preserve_clean_continuations() {
    shell_rows_support::replay("shell_output_chunks_preserve_clean_continuations");
}
#[test]
fn shell_completion_selects_cancel_error_or_success() {
    shell_rows_support::replay("shell_completion_selects_cancel_error_or_success");
}

#[test]
fn shell_output_keeps_whitespace_and_blank_entries() {
    shell_rows_support::replay("shell_output_keeps_whitespace_and_blank_entries");
}

#[test]
fn shell_preview_counts_logical_lines_after_context_tail() {
    shell_rows_support::replay("shell_preview_counts_logical_lines_after_context_tail");
}

#[test]
fn shell_collapsed_uses_render_width_not_terminal_columns() {
    shell_rows_support::replay("shell_collapsed_uses_render_width_not_terminal_columns");
}

#[test]
fn shell_preview_reflows_when_render_width_changes() {
    shell_rows_support::replay("shell_preview_reflows_when_render_width_changes");
}

#[test]
fn shell_visual_tail_limits_wrapped_rows_without_false_hint() {
    shell_rows_support::replay("shell_visual_tail_limits_wrapped_rows_without_false_hint");
}

#[test]
fn shell_visual_tail_keeps_unicode_tabs_and_word_wrapping() {
    shell_rows_support::replay("shell_visual_tail_keeps_unicode_tabs_and_word_wrapping");
}

#[test]
fn shell_completion_can_replace_status_without_restarting_loader() {
    shell_rows_support::replay("shell_completion_can_replace_status_without_restarting_loader");
}

#[test]
fn shell_external_truncation_requires_a_nonempty_artifact_path() {
    shell_rows_support::replay("shell_external_truncation_requires_a_nonempty_artifact_path");
}

#[test]
fn shell_context_tail_applies_line_and_utf8_byte_boundaries() {
    shell_rows_support::replay("shell_context_tail_applies_line_and_utf8_byte_boundaries");
}

#[test]
fn shell_notice_order_keeps_hint_before_status_before_artifact() {
    shell_rows_support::replay("shell_notice_order_keeps_hint_before_status_before_artifact");
}

#[test]
fn shell_excluded_command_stays_dim_when_rebuilt() {
    shell_rows_support::replay("shell_excluded_command_stays_dim_when_rebuilt");
}

#[test]
fn shell_live_borders_and_rebuilds_observe_theme_publication() {
    shell_rows_support::replay("shell_live_borders_and_rebuilds_observe_theme_publication");
}

#[test]
fn shell_expand_hints_sample_keys_only_when_rebuilt() {
    shell_rows_support::replay("shell_expand_hints_sample_keys_only_when_rebuilt");
}

#[test]
fn shell_running_rows_reuse_loader_until_completion() {
    shell_rows_support::replay("shell_running_rows_reuse_loader_until_completion");
}

#[test]
fn shell_invalidation_and_same_expansion_rebuild_display() {
    shell_rows_support::replay("shell_invalidation_and_same_expansion_rebuild_display");
}

#[test]
fn shell_container_children_keep_inherited_identity() {
    shell_rows_support::replay("shell_container_children_keep_inherited_identity");
}

#[test]
fn shell_running_preview_suppresses_completion_notices() {
    shell_rows_support::replay("shell_running_preview_suppresses_completion_notices");
}

#[test]
fn shell_exit_status_uses_decimal_and_exponent_spelling() {
    shell_rows_support::replay("shell_exit_status_uses_decimal_and_exponent_spelling");
}

#[test]
fn shell_narrow_viewport_uses_the_toolkit_invariant() {
    shell_rows_support::replay("shell_narrow_viewport_uses_the_toolkit_invariant");
}

#[test]
fn shell_default_themes_use_existing_state_roles() {
    shell_rows_support::replay("shell_default_themes_use_existing_state_roles");
}

#[test]
fn shell_last_retained_loader_owner_controls_teardown() {
    let _guard = shell_rows_support::registry_guard();
    let runtime = Rc::new(shell_rows_support::runtime::Observer::default());
    let terminal = recording_terminal::RecordingTerminal::new(40, 24);
    let ui = TUI::new(
        terminal.handle(),
        runtime.clone(),
        TerminalImage::default(),
        None,
    );
    let state = published::unpublished();
    let component = BashExecutionComponent::new("owner".into(), ui.clone(), state.theme(), false);
    let children = component.children();
    drop(component);
    assert_eq!(runtime.timers.get(), 1);
    for result in runtime.clock.advance_to(manual_runtime::ms(80)) {
        result.unwrap();
    }
    assert!(
        children
            .borrow()
            .iter()
            .flat_map(|child| child.render(40))
            .any(|row| row.contains('⠙'))
    );
    let old = children.replace(vec![]);
    drop(old);
    assert_eq!(runtime.timers.get(), 0);
    let redraws = runtime.redraws.get();
    for result in runtime.clock.advance_to(manual_runtime::ms(240)) {
        result.unwrap();
    }
    assert_eq!(runtime.redraws.get(), redraws);
    assert_eq!(Rc::strong_count(&runtime), 2);
    drop(ui);
}

#[test]
fn shell_unpublished_theme_preserves_plain_text_hints() {
    let _guard = shell_rows_support::registry_guard();
    shell_rows_support::default_bindings();
    let runtime = manual_runtime::ManualRuntime::default();
    let terminal = recording_terminal::RecordingTerminal::new(60, 24);
    let ui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    let state = published::unpublished();
    let component = BashExecutionComponent::new("plain".into(), ui, state.theme(), false);
    let initial = component.render(60);
    assert_eq!(
        initial[2],
        format!(" \x1b[1m$ plain\x1b[22m{}", " ".repeat(52))
    );
    assert_eq!(
        initial[4],
        format!(" ⠋ Running... (esc to cancel){}", " ".repeat(31))
    );
    component
        .append_output(
            &(0..22)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
    component.set_complete(None, false, None, None);
    assert_eq!(
        component.render(60)[24],
        format!(" ... 2 more lines (ctrl+o to expand){}", " ".repeat(24))
    );
}

#[test]
fn shell_try_render_preserves_child_rows_and_errors() {
    struct FallibleChild(std::cell::Cell<bool>);
    impl Component for FallibleChild {
        fn render(&self, _: usize) -> Vec<String> {
            vec!["infallible fallback".into()]
        }

        fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
            if self.0.get() {
                Err(Box::new(std::io::Error::other("child render failed")))
            } else {
                Ok(vec![format!("fallible child at width {width}")])
            }
        }
    }
    let _guard = shell_rows_support::registry_guard();
    let runtime = manual_runtime::ManualRuntime::default();
    let terminal = recording_terminal::RecordingTerminal::new(60, 24);
    let ui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    let state = published::unpublished();
    let component = BashExecutionComponent::new("child".into(), ui, state.theme(), false);
    component.clear();
    let child = Rc::new(FallibleChild(std::cell::Cell::new(false)));
    component.add_child(child.clone());
    assert_eq!(
        component.try_render(37).unwrap(),
        ["fallible child at width 37"]
    );
    child.0.set(true);
    let error = component.try_render(37).unwrap_err();
    assert_eq!(error.to_string(), "child render failed");
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::Other
    );
}

#[test]
fn shell_expand_and_collapse_hints_close_in_the_muted_colour() {
    let _guard = shell_rows_support::registry_guard();
    shell_rows_support::default_bindings();
    let runtime = manual_runtime::ManualRuntime::default();
    let terminal = recording_terminal::RecordingTerminal::new(60, 24);
    let ui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    let theme = published::theme(0);
    let (_state, live) = published::published(&theme);
    let component = BashExecutionComponent::new("hint".into(), ui, live.clone(), false);
    component
        .append_output(
            &(0..22)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
    component.set_complete(None, false, None, None);
    let muted = |text| theme.fg(&maestro_theme::ThemeColor::Muted, text).unwrap();
    let hint = |text| maestro_app::key_hint("app.tools.expand", text, &live).unwrap();
    let collapsed = format!(
        "{}{}{}",
        muted("... 2 more lines ("),
        hint("to expand"),
        muted(")")
    );
    assert!(
        component
            .render(60)
            .iter()
            .any(|row| row.contains(&collapsed)),
        "{:?}",
        component.render(60)
    );
    component.set_expanded(true);
    let expanded = format!("{}{}{}", muted("("), hint("to collapse"), muted(")"));
    assert!(
        component
            .render(60)
            .iter()
            .any(|row| row.contains(&expanded)),
        "{:?}",
        component.render(60)
    );
}

//! Sign-in dialog behavior against recorded terminal output.
#![cfg(test)]
use crate::keybindings;
#[path = "login_support/mod.rs"]
mod login_support;
use crate::manual_runtime;
use crate::published;
#[test]
fn login_title_uses_registry_and_override_precedence() {
    login_support::replay("login_title_uses_registry_and_override_precedence");
}
#[test]
fn authorization_rows_keep_links_hints_and_instructions() {
    login_support::replay("authorization_rows_keep_links_hints_and_instructions");
}
#[test]
fn authorization_replaces_content_without_resetting_input() {
    login_support::replay("authorization_replaces_content_without_resetting_input");
}
#[test]
fn manual_input_appends_and_starts_empty() {
    login_support::replay("manual_input_appends_and_starts_empty");
}
#[test]
fn prompt_input_resets_freezes_and_submits_exact_text() {
    login_support::replay("prompt_input_resets_freezes_and_submits_exact_text");
}
#[test]
fn sequential_prompts_freeze_submitted_rows_and_share_the_input() {
    login_support::replay("sequential_prompts_freeze_submitted_rows_and_share_the_input");
}
#[test]
fn information_replaces_rows_and_keeps_caller_text() {
    login_support::replay("information_replaces_rows_and_keeps_caller_text");
}
#[test]
fn information_keeps_hidden_pending_input() {
    login_support::replay("information_keeps_hidden_pending_input");
}
#[test]
fn waiting_appends_spacer_message_and_cancel_hint() {
    login_support::replay("waiting_appends_spacer_message_and_cancel_hint");
}
#[test]
fn progress_appends_only_message_and_requests_render() {
    login_support::replay("progress_appends_only_message_and_requests_render");
}
#[test]
fn pending_input_rejects_after_signal_abort() {
    login_support::replay("pending_input_rejects_after_signal_abort");
    login_support::cancellation_order();
}
#[test]
fn completion_reports_cancellation_once() {
    login_support::replay("completion_reports_cancellation_once");
}
#[test]
fn login_callbacks_allow_reentry_without_cycles() {
    login_support::replay("login_callbacks_allow_reentry_without_cycles");
}
#[test]
fn login_child_cancel_uses_same_completion() {
    login_support::replay("login_child_cancel_uses_same_completion");
}
#[test]
fn login_bindings_route_cancel_before_input_submission() {
    login_support::replay("login_bindings_route_cancel_before_input_submission");
}
#[test]
fn login_child_submission_uses_retained_input() {
    login_support::replay("login_child_submission_uses_retained_input");
}
#[test]
fn login_focus_forwards_to_retained_input() {
    login_support::replay("login_focus_forwards_to_retained_input");
}
#[test]
fn login_theme_capture_and_live_borders_are_distinct() {
    login_support::replay("login_theme_capture_and_live_borders_are_distinct");
}
#[test]
fn login_container_edits_preserve_retained_content() {
    login_support::replay("login_container_edits_preserve_retained_content");
}
#[test]
fn login_wrapping_and_escapes_use_existing_text() {
    login_support::replay("login_wrapping_and_escapes_use_existing_text");
}
#[test]
fn browser_launcher_receives_literal_url_without_shell_expansion() {
    login_support::replay("browser_launcher_receives_literal_url_without_shell_expansion");
}
use crate::recording_terminal;
use maestro_chat::{LoginDialogComponent, LoginDialogOptions, LoginDialogPlatform};
use maestro_tui::{Component, Focusable, tui::InputHandler};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Poll, Waker},
};
#[test]
fn application_provider_lookup_reaches_default_title() {
    let provider = maestro_app::presentation_data::get_oauth_providers()
        .into_iter()
        .next()
        .unwrap();
    let (_, theme) = published::published(&login_support::colors(0));
    let dialog = LoginDialogComponent::new(
        Rc::new(|| {}),
        provider.id(),
        Rc::new(|_, _| {}),
        LoginDialogOptions {
            theme,
            provider_name_override: None,
            title_override: None,
            platform: LoginDialogPlatform::default(),
            provider_name_lookup: None,
            launch_browser: Some(Rc::new(|_| {})),
        },
    )
    .unwrap();
    assert!(dialog.render(80)[1].contains(&format!("Login to {}", provider.name())));
}
#[cfg(unix)]
#[test]
fn native_browser_launch_is_detached_and_uses_one_literal_argument() {
    login_support::native::launch();
}
#[test]
fn input_futures_are_eager_owned_and_do_not_cycle() {
    let _bindings = keybindings::bindings_guard();
    let runtime = manual_runtime::ManualRuntime::new();
    let terminal = recording_terminal::RecordingTerminal::new(80, 24);
    let terminal_handle = terminal.handle();
    let weak_terminal = Rc::downgrade(&terminal_handle);
    let writer = maestro_tui::TUI::new(
        terminal_handle,
        runtime.handle(),
        maestro_tui::TerminalImage::new(|_| None, || 1),
        Some(true),
    );
    let (_, theme) = published::published(&login_support::colors(0));
    let completion = Rc::new(Cell::new(0));
    let count = completion.clone();
    let dialog = themed_panel(
        theme,
        writer.weak_render_request(),
        Rc::new(move |_, _| count.set(count.get() + 1)),
    );
    writer.add_child(dialog.clone());
    writer.set_focus(Some(dialog.clone()));
    let mut future = dialog
        .show_prompt(&String::from("Temporary prompt"), None)
        .unwrap();
    let input = login_support::retained_input(&dialog);
    assert!(input.focusable().unwrap().focus_flag().get());
    input.input_handler().unwrap().handle_input("first");
    input.input_handler().unwrap().handle_input("\r");
    assert_eq!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Ok("first".into()))
    );
    drop(dialog.show_manual_input("Unobserved").unwrap());
    assert_eq!(completion.get(), 0);
    let mut pending = dialog.show_manual_input("Retained").unwrap();
    let weak = Rc::downgrade(&dialog);
    writer.stop().unwrap();
    writer.clear();
    writer.set_focus(None);
    drop(dialog);
    drop(writer);
    assert!(weak.upgrade().is_none());
    assert!(weak_terminal.upgrade().is_none());
    assert_eq!(runtime.pending(), 0);
    input.input_handler().unwrap().handle_input("second");
    input.input_handler().unwrap().handle_input("\r");
    assert_eq!(
        pending
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Ok("second".into()))
    );
    assert_eq!(completion.get(), 0);
    let weak_input = Rc::downgrade(&input);
    drop(input);
    assert!(weak_input.upgrade().is_none());
}
#[test]
fn blank_manual_submission_keeps_input_and_cancellation_pending() {
    let _bindings = keybindings::bindings_guard();
    for cancel in [false, true] {
        let (_, theme) = published::published(&login_support::colors(0));
        let dialog = themed_panel(theme, Rc::new(|| {}), Rc::new(|_, _| {}));
        let mut reply = dialog.show_manual_input("Paste redirect").unwrap();
        dialog.handle_input("\r");
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(reply.as_mut().poll(&mut context), Poll::Pending);
        if cancel {
            dialog.handle_input("\x1b");
            assert_eq!(
                reply.as_mut().poll(&mut context),
                Poll::Ready(Err(maestro_chat::LoginDialogError::Cancelled))
            );
        } else {
            dialog.handle_input("redirect-code");
            dialog.handle_input("\r");
            assert_eq!(
                reply.as_mut().poll(&mut context),
                Poll::Ready(Ok("redirect-code".into()))
            );
        }
    }
}
#[test]
fn login_rows_fit_one_cell_viewport() {
    let _bindings = keybindings::bindings_guard();
    let (dialog, _) =
        login_support::controlled(LoginDialogPlatform::default(), Some(Rc::new(|_| {})));
    dialog
        .show_auth("https://auth.example.test/界", Some("Authorize"))
        .unwrap();
    drop(dialog.show_prompt("Prompt", Some("Example")).unwrap());
    dialog.focus_flag().set(true);
    dialog.handle_input("界");
    dialog.show_waiting("Waiting").unwrap();
    dialog.show_progress("Progress").unwrap();
    for row in dialog.render(1) {
        assert!(maestro_tui::visible_width(&row) <= 1);
    }
}
#[test]
fn login_dialog_uses_current_brand_roles() {
    let _bindings = keybindings::bindings_guard();
    for mode in ["dark", "light"] {
        let theme = Rc::new(
            maestro_theme::load_theme_from_path(
                &format!(
                    "{}/../maestro-theme/assets/theme/{mode}.json",
                    env!("CARGO_MANIFEST_DIR")
                ),
                Some(maestro_theme::ColorMode::Truecolor),
            )
            .unwrap(),
        );
        let (_, live) = published::published(&theme);
        let dialog = themed_panel(live.clone(), Rc::new(|| {}), Rc::new(|_, _| {}));
        dialog
            .show_auth("https://auth.example.test", Some("Instructions"))
            .unwrap();
        drop(dialog.show_manual_input("Manual").unwrap());
        drop(dialog.show_prompt("Prompt", Some("Example")).unwrap());
        dialog.focus_flag().set(true);
        dialog.show_waiting("Waiting").unwrap();
        dialog.show_progress("Progress").unwrap();
        let rows = dialog.render(80);
        assert_brand_rows(&rows, &theme, &live);
        dialog.show_info(&["Information".into()]).unwrap();
        let rows = dialog.render(80);
        assert_eq!(rows[3], format!(" Information{}", " ".repeat(68)));
        assert!(
            rows[5]
                .contains(&maestro_app::key_hint("tui.select.cancel", "to close", &live).unwrap())
        );
    }
}

/// Construct a styled panel retaining only supplied caller effects.
fn themed_panel(
    theme: maestro_theme::LiveTheme,
    request: Rc<dyn Fn()>,
    completion: Completion,
) -> Rc<LoginDialogComponent> {
    Rc::new(
        LoginDialogComponent::new(
            request,
            "p",
            completion,
            LoginDialogOptions {
                theme,
                provider_name_override: None,
                title_override: None,
                platform: LoginDialogPlatform::Other,
                provider_name_lookup: None,
                launch_browser: Some(Rc::new(|_| {})),
            },
        )
        .unwrap(),
    )
}
/// Every captured foreground and live border comes from the published token pack.
fn assert_brand_rows(
    rows: &[String],
    theme: &maestro_theme::Theme,
    live: &maestro_theme::LiveTheme,
) {
    for (role, text) in [
        (maestro_theme::ThemeColor::Warning, "Instructions"),
        (maestro_theme::ThemeColor::Dim, "Manual"),
        (maestro_theme::ThemeColor::Text, "Prompt"),
        (maestro_theme::ThemeColor::Dim, "e.g., Example"),
        (maestro_theme::ThemeColor::Dim, "Waiting"),
        (maestro_theme::ThemeColor::Dim, "Progress"),
    ] {
        assert!(
            rows.iter()
                .any(|row| row.contains(&theme.fg(&role, text).unwrap()))
        );
    }
    assert!(
        rows[1].contains(
            &theme
                .fg(
                    &maestro_theme::ThemeColor::Accent,
                    &maestro_theme::Theme::bold("Login to p", true)
                )
                .unwrap()
        )
    );
    let url = "https://auth.example.test";
    for (role, text) in [
        (maestro_theme::ThemeColor::Accent, url),
        (maestro_theme::ThemeColor::Dim, "Ctrl+click to open"),
    ] {
        let link = format!("\x1b]8;;{url}\x07{text}\x1b]8;;\x07");
        assert!(
            rows.iter()
                .any(|row| row.contains(&theme.fg(&role, &link).unwrap()))
        );
    }
    assert!(rows.iter().any(|row| {
        row.contains(&maestro_app::key_hint("tui.select.confirm", "to submit", live).unwrap())
    }));
    assert_eq!(
        rows[0],
        theme
            .fg(&maestro_theme::ThemeColor::Border, &"─".repeat(80))
            .unwrap()
    );
    assert_eq!(rows.last().unwrap(), &rows[0]);
    assert!(
        rows.iter()
            .any(|row| row.contains(maestro_tui::CURSOR_MARKER))
    );
    assert!(rows.iter().any(|row| {
        row.contains(&maestro_app::key_hint("tui.select.cancel", "to cancel", live).unwrap())
    }));
}

/// Caller notification retained by a styled panel.
type Completion = Rc<dyn Fn(bool, Option<&str>)>;

/// Rows of the dialog at width 120, without trailing blanks.
fn rows(dialog: &maestro_chat::LoginDialogComponent) -> Vec<String> {
    use maestro_tui::Component;
    dialog
        .render(120)
        .into_iter()
        .map(|row| row.trim_end().to_owned())
        .collect()
}

/// Rows whose text, apart from escapes, holds `> value`.
fn echoes(rows: &[String], value: &str) -> usize {
    rows.iter()
        .filter(|row| row.contains(&format!("> {value}")))
        .count()
}

/// The text a settled input future produced.
fn settled(
    input: maestro_chat::LoginInputFuture,
) -> Result<String, maestro_chat::LoginDialogError> {
    use std::task::{Context, Poll, Waker};
    let mut input = input;
    match input.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("the input did not settle"),
    }
}

#[test]
fn submitted_prompt_rows_stay_frozen_while_a_later_prompt_is_active() {
    use maestro_tui::tui::InputHandler as _;
    let _guard = keybindings::bindings_guard();
    let (dialog, _) = login_support::controlled(
        maestro_chat::LoginDialogPlatform::Other,
        Some(std::rc::Rc::new(|_| {})),
    );
    let first = dialog
        .show_prompt("First prompt:", Some("first-value"))
        .unwrap();
    dialog.handle_input("first-value");
    dialog.handle_input("\r");
    assert_eq!(settled(first).as_deref(), Ok("first-value"));
    let second = dialog.show_prompt("Second prompt:", None).unwrap();
    dialog.handle_input("second-secret-demo");
    let rows = rows(&dialog);
    let text = rows.join("\n");
    assert!(text.contains("First prompt:") && text.contains("Second prompt:"));
    assert_eq!(echoes(&rows, "first-value"), 1);
    assert_eq!(echoes(&rows, "second-secret-demo"), 1);
    dialog.handle_input("\r");
    assert_eq!(settled(second).as_deref(), Ok("second-secret-demo"));
}

#[test]
fn submitted_manual_input_stays_frozen_and_the_next_prompt_starts_empty() {
    use maestro_tui::tui::InputHandler as _;
    let _guard = keybindings::bindings_guard();
    let (dialog, _) = login_support::controlled(
        maestro_chat::LoginDialogPlatform::Other,
        Some(std::rc::Rc::new(|_| {})),
    );
    let manual = dialog.show_manual_input("Paste callback URL:").unwrap();
    dialog.handle_input("callback-value");
    dialog.handle_input("\r");
    assert_eq!(settled(manual).as_deref(), Ok("callback-value"));
    let prompt = dialog.show_prompt("Second prompt:", None).unwrap();
    let rows = rows(&dialog);
    assert_eq!(echoes(&rows, "callback-value"), 1);
    dialog.handle_input("second-secret-demo");
    let rows = self::rows(&dialog);
    assert_eq!(echoes(&rows, "callback-value"), 1);
    assert_eq!(echoes(&rows, "second-secret-demo"), 1);
    dialog.handle_input("\r");
    assert_eq!(settled(prompt).as_deref(), Ok("second-secret-demo"));
}

#[test]
fn manual_input_started_after_a_prompt_starts_empty() {
    use maestro_tui::tui::InputHandler as _;
    let _guard = keybindings::bindings_guard();
    let (dialog, _) = login_support::controlled(
        maestro_chat::LoginDialogPlatform::Other,
        Some(std::rc::Rc::new(|_| {})),
    );
    let prompt = dialog.show_prompt("First prompt:", None).unwrap();
    dialog.handle_input("typed-before-submit");
    dialog.handle_input("\r");
    assert_eq!(settled(prompt).as_deref(), Ok("typed-before-submit"));
    let manual = dialog.show_manual_input("Paste callback URL:").unwrap();
    dialog.handle_input("\r");
    let rows = rows(&dialog);
    assert_eq!(echoes(&rows, "typed-before-submit"), 1);
    drop(manual);
}

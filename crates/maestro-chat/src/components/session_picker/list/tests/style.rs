//! Private state observations through the public renderer.
use super::super::super::SessionList;
use super::session_picker;
use maestro_tui::interface::Focusable;
/// Configures existing list state without adding dispatch or public setters.
fn configure(list: &SessionList, path: bool, confirmation: Option<&str>, selected: Option<usize>) {
    list.show_path.set(path);
    list.confirming.replace(confirmation.map(str::to_owned));
    if let Some(selected) = selected {
        if selected == usize::MAX {
            list.input.focus_flag().set(false);
        } else {
            list.selected.set(selected);
        }
    }
}
#[test]
fn session_row_paths_counts_and_cwd_follow_independent_flags() {
    session_picker::list(
        "session_row_paths_counts_and_cwd_follow_independent_flags",
        Some(configure),
    );
}

#[test]
fn session_row_style_precedence_and_selected_background() {
    session_picker::list(
        "session_row_style_precedence_and_selected_background",
        Some(configure),
    );
}

#[test]
fn session_list_focus_emits_search_cursor_on_focus_changes() {
    session_picker::list(
        "session_list_focus_emits_search_cursor_on_focus_changes",
        Some(configure),
    );
}

#[test]
fn session_date_and_home_observations_refresh_each_render() {
    session_picker::list(
        "session_date_and_home_observations_refresh_each_render",
        Some(configure),
    );
}

#[cfg(unix)]
#[test]
fn session_metadata_controls_are_displayed_without_terminal_commands() {
    session_picker::list(
        "session_metadata_controls_are_displayed_without_terminal_commands",
        Some(configure),
    );
}

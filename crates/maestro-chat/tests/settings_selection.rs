//! Settings changes through the retained public input list.
#![cfg(test)]
use crate::keybindings;
use crate::published;
use crate::selector;
use crate::settings_selection_support;
use maestro_tui::Component;
use selector::Empty;
use settings_selection_support::*;

#[test]
fn settings_rows_keep_text_order_and_initial_values() {
    let _guard = bindings_guard();
    for (_, expected) in
        cases::<Empty, Inventory>("settings_rows_keep_text_order_and_initial_values")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme,
            &no_images(),
        );
        assert_eq!(
            frame(&selector, &events, 80),
            Frame {
                lines: expected.lines,
                events: expected.events
            }
        );
        assert_eq!(expected.items.len(), 21);
        for (index, item) in expected.items.iter().enumerate() {
            assert_inventory_row(&selector, item);
            let list = selector.get_settings_list();
            if index < 20 {
                input(&**list, &["\x1b[B"]);
            }
        }
    }
}

#[test]
fn boolean_changes_report_only_the_selected_setting() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in
        cases::<BooleanInput, ThreeFrames>("boolean_changes_report_only_the_selected_setting")
    {
        let events = Events::default();
        let mut cfg = config();
        set_boolean(&mut cfg, &query.field, query.current);
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, &query.id);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.one);
        input(&**selector.get_settings_list(), &[" "]);
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn delivery_modes_remain_independent_on_constructor_and_change() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in cases::<DeliveryInput, ThreeFrames>(
        "delivery_modes_remain_independent_on_constructor_and_change",
    ) {
        let events = Events::default();
        let mut cfg = config();
        set_delivery(&mut cfg, &query.field, &query.current);
        set_delivery(
            &mut cfg,
            &query.sibling,
            if query.current == "all" {
                "one-at-a-time"
            } else {
                "all"
            },
        );
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, &query.id);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.one);
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn transport_cycles_all_four_typed_choices() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in
        cases::<Current, BeforeAfter>("transport_cycles_all_four_typed_choices")
    {
        let events = Events::default();
        let mut cfg = config();
        cfg.transport = transport(&query.current);
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "transport");
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r", "\r", "\r", "\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn escape_action_and_tree_filter_cover_every_choice() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in
        cases::<ActionInput, BeforeAfter>("escape_action_and_tree_filter_cover_every_choice")
    {
        let events = Events::default();
        let mut cfg = config();
        set_action(&mut cfg, &query.field, &query.current);
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, &query.id);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        for _ in 0..if query.id == "tree-filter-mode" { 5 } else { 3 } {
            input(&**selector.get_settings_list(), &["\r"]);
        }
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn settings_search_dispatches_visible_rows_and_cancel() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in
        cases::<Query, BeforeAfter>("settings_search_dispatches_visible_rows_and_cancel")
    {
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        input(&**selector.get_settings_list(), &[&query.query]);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r", "\x1b"]);
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn settings_selected_descriptions_and_scroll_are_exact() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in
        cases::<Id, Frame>("settings_selected_descriptions_and_scroll_are_exact")
    {
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, &query.id);
        assert_eq!(frame(&selector, &events, 80), expected);
    }
}

#[test]
fn settings_layout_reuses_terminal_width_rules() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in cases::<Layout, Frame>("settings_layout_reuses_terminal_width_rules") {
        let events = Events::default();
        let mut cfg = config();
        if query.surface.is_some() {
            cfg.available_themes = vec![
                "a".repeat(45),
                "dark".into(),
                "界e\u{301}👩\u{200d}💻".into(),
            ];
        }
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        if let Some(surface) = query.surface {
            select(&selector, &surface);
            input(&**selector.get_settings_list(), &["\r"]);
        }
        assert_eq!(frame(&selector, &events, query.width), expected);
    }
}

#[test]
fn thinking_submenu_keeps_descriptions_and_typed_confirmation() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in
        cases::<Current, BeforeAfter>("thinking_submenu_keeps_descriptions_and_typed_confirmation")
    {
        let events = Events::default();
        let mut cfg = config();
        cfg.thinking_level = query.current.parse().unwrap();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "thinking");
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn thinking_submenu_handles_empty_singleton_duplicates_and_missing_current() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in cases::<ThinkingInput, Frames>(
        "thinking_submenu_handles_empty_singleton_duplicates_and_missing_current",
    ) {
        let events = Events::default();
        let mut cfg = config();
        cfg.thinking_level = query.current;
        cfg.available_thinking_levels = query.available;
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "thinking");
        input(&**selector.get_settings_list(), &["\r"]);
        let mut observed = vec![frame(&selector, &events, 80)];
        for key in ["\x1b[B", "\r", "\x1b"] {
            input(&**selector.get_settings_list(), &[key]);
            observed.push(frame(&selector, &events, 80));
        }
        assert_eq!(observed, expected.frames);
    }
}

#[test]
fn thinking_cancel_retains_value_and_reopens_same_selection() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (_, expected) in
        cases::<Empty, Frame>("thinking_cancel_retains_value_and_reopens_same_selection")
    {
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "thinking");
        input(
            &**selector.get_settings_list(),
            &["\r", "\x1b[B", "\x1b", "\r"],
        );
        assert_eq!(frame(&selector, &events, 80), expected);
    }
}

#[test]
fn submenus_cap_visible_options_at_ten() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in cases::<Window, Frame>("submenus_cap_visible_options_at_ten") {
        let events = Events::default();
        let mut cfg = config();
        assert_eq!(query.n, query.available.len());
        cfg.available_thinking_levels = query.available;
        cfg.thinking_level = maestro_app::ThinkingLevel::Minimal;
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "thinking");
        input(&**selector.get_settings_list(), &["\r"]);
        for _ in 0..6 {
            input(&**selector.get_settings_list(), &["\x1b[B"]);
        }
        assert_eq!(frame(&selector, &events, 80), expected);
    }
}

#[test]
fn theme_navigation_preview_cancel_and_confirm_follow_callback_order() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in cases::<ThemeInput, Frames>(
        "theme_navigation_preview_cancel_and_confirm_follow_callback_order",
    ) {
        let events = Events::default();
        let mut cfg = config();
        cfg.available_themes = query.available;
        cfg.current_theme = query.current;
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(query.preview),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "theme");
        input(&**selector.get_settings_list(), &["\r"]);
        let mut observed = vec![frame(&selector, &events, 80)];
        for key in ["\x1b[B", "\x1b[A", "\x1b", "\r", "\x1b[B", "\r"] {
            input(&**selector.get_settings_list(), &[key]);
            observed.push(frame(&selector, &events, 80));
        }
        assert_eq!(observed, expected.frames);
    }
}

#[test]
fn theme_cancel_restores_latest_committed_value() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (_, expected) in cases::<Empty, Frame>("theme_cancel_restores_latest_committed_value") {
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "theme");
        input(
            &**selector.get_settings_list(),
            &["\r", "\x1b[B", "\r", "\r", "\x1b[A", "\x1b"],
        );
        assert_eq!(frame(&selector, &events, 80), expected);
    }
}

#[test]
fn warning_edits_keep_unknown_members_snapshots_and_reopened_state() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in cases::<WarningsInput, Frames>(
        "warning_edits_keep_unknown_members_snapshots_and_reopened_state",
    ) {
        let events = Events::default();
        let mut cfg = config();
        cfg.warnings = query.warnings;
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, "warnings");
        input(&**selector.get_settings_list(), &["\r"]);
        let mut observed = vec![frame(&selector, &events, 80)];
        for key in ["\r", " ", "\x1b", "\r", "\r", "\x1b"] {
            input(&**selector.get_settings_list(), &[key]);
            observed.push(frame(&selector, &events, 80));
        }
        assert_eq!(observed, expected.frames);
    }
}

#[test]
fn settings_live_colors_keep_captured_cursor_and_submenu_chrome() {
    let _guard = bindings_guard();
    for (_, expected) in
        cases::<Empty, Colors>("settings_live_colors_keep_captured_cursor_and_submenu_chrome")
    {
        let events = Events::default();
        let (state, theme) = published::published(&colors(0));
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme,
            &no_images(),
        );
        assert_eq!(frame(&selector, &events, 80), expected.initial);
        select(&selector, "theme");
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.open);
        state.set_theme_instance(colors(4)).unwrap();
        selector.invalidate();
        assert_eq!(frame(&selector, &events, 80), expected.changed);
        input(&**selector.get_settings_list(), &["\x1b"]);
        assert_eq!(frame(&selector, &events, 80), expected.main);
    }
}

#[test]
fn settings_and_submenus_use_live_keybindings() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (query, expected) in cases::<Id, Frames>("settings_and_submenus_use_live_keybindings") {
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        select(&selector, &query.id);
        keybindings::bindings(["k", "j", "x", "q"]);
        let mut observed = vec![];
        for key in ["x", "\r", "\x1b[B", "\x1b[120;2u", "j", "x", "q"] {
            input(&**selector.get_settings_list(), &[key]);
            observed.push(frame(&selector, &events, 80));
        }
        assert_eq!(observed, expected.frames);
        maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
            maestro_tui::TUI_KEYBINDINGS.clone(),
            Vec::default(),
        ));
    }
}

#[test]
fn settings_list_handle_survives_chrome_removal() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (_, expected) in cases::<Empty, Children>("settings_list_handle_survives_chrome_removal") {
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme.clone(),
            &no_images(),
        );
        let list = selector.get_settings_list().clone();
        assert_eq!(
            std::rc::Rc::ptr_eq(&list, selector.get_settings_list()),
            expected.identical
        );
        let children = selector.children();
        selector.clear();
        assert_eq!(selector.render(80), expected.cleared);
        input(&*list, &["\r"]);
        assert_eq!(list.render(80), expected.retained);
        selector.set_children(children);
        assert_eq!(frame(&selector, &events, 80), expected.restored);
    }
}

#[test]
fn warning_callback_reenters_after_state_is_committed() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    for (_, expected) in cases::<Empty, Frame>("warning_callback_reenters_after_state_is_committed")
    {
        let events = Events::default();
        let mut callbacks = events.callbacks(true);
        let original = callbacks.on_warnings_change.clone();
        let target = std::rc::Rc::new(std::cell::RefCell::new(std::rc::Weak::<
            maestro_tui::SettingsList,
        >::new()));
        callbacks.on_warnings_change = observing(original, target.clone(), events.clone());
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            callbacks,
            theme.clone(),
            &no_images(),
        );
        target.replace(std::rc::Rc::downgrade(selector.get_settings_list()));
        select(&selector, "warnings");
        input(&**selector.get_settings_list(), &["\r", "\r", "\x1b"]);
        assert_eq!(frame(&selector, &events, 80), expected);
    }
}

#[test]
fn settings_callbacks_release_after_last_list_handle() {
    let _guard = bindings_guard();
    let (_, theme) = published::published(&colors(0));
    let events = Events::default();
    let captured = std::rc::Rc::new(());
    let released = std::rc::Rc::downgrade(&captured);
    let mut callbacks = events.callbacks(true);
    callbacks.on_auto_compact_change = std::rc::Rc::new(move |_| {
        assert_eq!(std::rc::Rc::strong_count(&captured), 1);
        true
    });
    let parent = std::rc::Rc::new(maestro_chat::SettingsSelectorComponent::new(
        config(),
        callbacks,
        theme,
        &no_images(),
    ));
    let owner = std::rc::Rc::downgrade(&parent);
    let list = parent.get_settings_list().clone();
    select(&parent, "warnings");
    input(&*list, &["\r"]);
    drop(parent);
    assert!(owner.upgrade().is_none());
    assert!(released.upgrade().is_some());
    input(&*list, &["\r", "\x1b"]);
    assert!(!list.render(80).is_empty());
    drop(list);
    assert!(released.upgrade().is_none());
}

#[test]
fn refused_warning_and_theme_choices_restore_their_previous_view() {
    let _guard = bindings_guard();
    for id in ["warnings", "theme"] {
        let events = Events::default();
        let (_, theme) = published::published(&colors(0));
        let mut callbacks = events.callbacks(true);
        callbacks.on_warnings_change = std::rc::Rc::new(|_| false);
        callbacks.on_theme_change = std::rc::Rc::new(|_| false);
        let selector =
            maestro_chat::SettingsSelectorComponent::new(config(), callbacks, theme, &no_images());
        select(&selector, id);
        let main = selector.render(80);
        let list = selector.get_settings_list();
        input(&**list, &["\r"]);
        let before = selector.render(80);
        if id == "warnings" {
            input(&**list, &["\r"]);
            assert_eq!(selector.render(80), before);
            input(&**list, &["\x1b", "\r"]);
            assert_eq!(selector.render(80), before, "reopened warning snapshot");
            input(&**list, &["\x1b"]);
        } else {
            input(&**list, &["\x1b[B", "\r"]);
            assert_eq!(
                events.get().last().unwrap(),
                &vec![serde_json::json!("preview"), serde_json::json!("dark")]
            );
        }
        assert_eq!(selector.render(80), main);
    }
}

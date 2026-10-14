//! Display settings through the retained public settings list.
#![cfg(test)]
#[path = "support/keybindings.rs"]
mod keybindings;
#[allow(dead_code, reason = "Shared published theme support.")]
#[path = "../../maestro-theme/tests/published/mod.rs"]
mod published;
mod settings_display_support;
#[allow(
    dead_code,
    reason = "Shared settings corpus owners serve both test targets."
)]
mod settings_selection_support;
use maestro_tui::Component;
use settings_display_support::*;
use settings_selection_support::*;

#[test]
fn display_rows_follow_terminal_capabilities() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<Protocol, Inventory>("display_rows_follow_terminal_capabilities")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme,
            &terminal(query.protocol.as_deref()),
        );
        assert_panel(&selector, &events, &expected, query.protocol.is_some());
    }
}

#[test]
fn display_booleans_notify_only_their_own_callback() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<Boolean, Frames>("display_booleans_notify_only_their_own_callback")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let mut cfg = config();
        for field in BOOLEAN_FIELDS {
            set_boolean(&mut cfg, field, !query.current);
        }
        set_boolean(&mut cfg, &query.field, query.current);
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme,
            &terminal(query.protocol.as_deref()),
        );
        select_display(&selector, &query.id, query.protocol.is_some());
        let mut observed = vec![frame(&selector, &events, 80)];
        for key in ["\r", " "] {
            input(&**selector.get_settings_list(), &[key]);
            observed.push(frame(&selector, &events, 80));
        }
        assert_eq!(observed, expected.frames);
    }
}

#[test]
fn numeric_settings_cycle_fixed_choices_and_wrap() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<Numeric, Frames>("numeric_settings_cycle_fixed_choices_and_wrap")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let mut cfg = config();
        set_numeric(&mut cfg, &query.field, &query.current);
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme,
            &terminal(Some("kitty")),
        );
        select_display(&selector, &query.id, true);
        let mut observed = vec![frame(&selector, &events, 80)];
        for _ in 1..expected.frames.len() {
            input(&**selector.get_settings_list(), &["\r"]);
            observed.push(frame(&selector, &events, 80));
        }
        assert_eq!(observed, expected.frames);
    }
}

#[test]
fn numeric_constructor_values_keep_supplied_spelling() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<Numeric, BeforeAfter>("numeric_constructor_values_keep_supplied_spelling")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let mut cfg = config();
        set_numeric(&mut cfg, &query.field, &query.current);
        let selector = maestro_chat::SettingsSelectorComponent::new(
            cfg,
            events.callbacks(true),
            theme,
            &terminal(Some("kitty")),
        );
        select_display(&selector, &query.id, true);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn display_activation_updates_list_before_callback() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<Id, BeforeAfter>("display_activation_updates_list_before_callback")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let mut callbacks = events.callbacks(true);
        let target = Target::default();
        observe_callback(&mut callbacks, &query.id, &target, &events);
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            callbacks,
            theme,
            &terminal(Some("kitty")),
        );
        target.replace(std::rc::Rc::downgrade(selector.get_settings_list()));
        select_display(&selector, &query.id, true);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r"]);
        assert_eq!(frame(&selector, &events, 80), expected.after);
        let released = std::rc::Rc::downgrade(selector.get_settings_list());
        drop(selector);
        assert!(released.upgrade().is_none());
    }
}

#[test]
fn display_capabilities_are_sampled_once() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<CapabilityChange, CapabilityFrames>("display_capabilities_are_sampled_once")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let terminal = terminal(query.initial.as_deref());
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme.clone(),
            &terminal,
        );
        assert_eq!(frame(&selector, &events, 80), expected.before);
        terminal.set_capabilities(capabilities(query.later.as_deref()));
        selector.invalidate();
        assert_eq!(frame(&selector, &events, 80), expected.after);
        let fresh_events = Events::default();
        let fresh = maestro_chat::SettingsSelectorComponent::new(
            config(),
            fresh_events.callbacks(true),
            theme,
            &terminal,
        );
        assert_panel(
            &fresh,
            &fresh_events,
            &expected.fresh,
            query.later.is_some(),
        );
    }
}

#[test]
fn display_search_uses_existing_keybindings() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<Search, SearchFrames>("display_search_uses_existing_keybindings")
    {
        maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
            maestro_tui::TUI_KEYBINDINGS.clone(),
            Vec::default(),
        ));
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme,
            &terminal(query.protocol.as_deref()),
        );
        if query.id.is_some() {
            keybindings::bindings(["k", "j", "x", "q"]);
        }
        input(&**selector.get_settings_list(), &[&query.query]);
        assert_eq!(frame(&selector, &events, 80), expected.before);
        input(&**selector.get_settings_list(), &["\r"]);
        if let Some(id) = query.id {
            assert_eq!(frame(&selector, &events, 80), expected.old_key.unwrap());
            input(&**selector.get_settings_list(), &["x", "q"]);
            assert_eq!(events.get()[0][0], callback_name(&id));
        } else {
            assert!(expected.old_key.is_none());
            input(&**selector.get_settings_list(), &["\x1b"]);
        }
        assert_eq!(frame(&selector, &events, 80), expected.after);
    }
}

#[test]
fn display_rows_preserve_styles_and_wrapping() {
    let _guard = bindings_guard();
    for (query, expected) in
        display_cases::<StyledInput, StyledFrames>("display_rows_preserve_styles_and_wrapping")
    {
        let (state, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = maestro_chat::SettingsSelectorComponent::new(
            config(),
            events.callbacks(true),
            theme,
            &terminal(Some("kitty")),
        );
        select_display(&selector, &query.id, true);
        assert_eq!(frame(&selector, &events, query.width), expected.initial);
        state.set_theme_instance(colors(4)).unwrap();
        selector.invalidate();
        assert_eq!(frame(&selector, &events, query.width), expected.changed);
        assert!(
            selector
                .render(query.width)
                .iter()
                .all(|line| maestro_tui::visible_width(line) <= query.width)
        );
    }
}

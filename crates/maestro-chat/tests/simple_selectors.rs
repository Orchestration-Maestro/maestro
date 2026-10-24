//! Public selector behavior against recorded terminal rows.
#![cfg(test)]
use crate::published;
#[path = "simple_selector_support/mod.rs"]
mod simple_selector_support;
#[cfg(not(target_arch = "wasm32"))]
use crate::keybindings;
#[cfg(not(target_arch = "wasm32"))]
use crate::manual_runtime;
#[cfg(target_arch = "wasm32")]
use crate::simple_selector_cases::*;
#[cfg(not(target_arch = "wasm32"))]
use keybindings::bindings;
use maestro_app::ThinkingLevel;
use maestro_chat::ThinkingSelectorComponent;
#[cfg(not(target_arch = "wasm32"))]
use maestro_tui::Component;
#[cfg(not(target_arch = "wasm32"))]
use simple_selector_support::*;
#[cfg(not(target_arch = "wasm32"))]
use std::{error::Error, rc::Rc};

#[test]
fn thinking_levels_keep_descriptions_and_current_selection() {
    let _bindings = bindings_guard();
    for (input, expected) in
        cases::<ThinkingInput, Snapshot>("thinking_levels_keep_descriptions_and_current_selection")
    {
        let (_, live) = published::published(&colors(0));
        let events = Events::default();
        let selector = ThinkingSelectorComponent::new(
            input.current,
            input.levels,
            events.thinking(),
            events.cancel(),
            live,
        );
        assert_eq!(
            snapshot(&selector, selector.get_select_list(), &events, 80),
            expected
        );
    }
}

#[test]
fn thinking_supplied_order_duplicates_and_missing_current() {
    let _bindings = bindings_guard();
    for (query, expected) in cases::<AvailableInput, BeforeAfter>(
        "thinking_supplied_order_duplicates_and_missing_current",
    ) {
        let (_, live) = published::published(&colors(0));
        let events = Events::default();
        let selector = ThinkingSelectorComponent::new(
            query.current,
            query.available,
            events.thinking(),
            events.cancel(),
            live,
        );
        let list = selector.get_select_list();
        assert_eq!(snapshot(&selector, list, &events, 80), expected.before);
        input(&**list, &["\r", "\x1b[B", "\r", "\x1b[A", "\x1b", "z"]);
        assert_eq!(snapshot(&selector, list, &events, 80), expected.after);
    }
}
#[test]
fn thinking_selection_reports_typed_levels() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<CurrentLevel, EventOutput>("thinking_selection_reports_typed_levels")
    {
        let (_, live) = published::published(&colors(0));
        let events = Events::default();
        let current: ThinkingLevel = query.current;
        let selector = ThinkingSelectorComponent::new(
            current,
            levels(),
            events.thinking(),
            events.cancel(),
            live,
        );
        input(
            &**selector.get_select_list(),
            &["\r", "\x1b[A", "\r", "\x1b[B", "\x1b"],
        );
        assert_eq!(events.get(), expected.events);
    }
}
#[test]
fn image_choices_render_and_report_both_boolean_values() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<ImageInput, BeforeAfter>("image_choices_render_and_report_both_boolean_values")
    {
        let (_, live) = published::published(&colors(0));
        let events = Events::default();
        let selector = maestro_chat::ShowImagesSelectorComponent::new(
            query.current,
            events.record("select"),
            events.cancel(),
            live,
        );
        let list = selector.get_select_list();
        assert_eq!(snapshot(&selector, list, &events, 80), expected.before);
        input(
            &**list,
            &["\r", "\x1b[B", "\r", "\x1b[A", "\r", "\x1b", "z"],
        );
        assert_eq!(snapshot(&selector, list, &events, 80), expected.after);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn theme_inventory_marks_only_the_current_name() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<ThemeInput, Snapshot>("theme_inventory_marks_only_the_current_name")
    {
        let (_scratch, state) = registry(&query.names);
        let events = Events::default();
        let selector = maestro_chat::ThemeSelectorComponent::new(
            &query.current,
            events.select(),
            events.cancel(),
            events.preview(),
            &state,
        )
        .unwrap();
        assert_eq!(
            snapshot(&selector, selector.get_select_list(), &events, 80),
            expected
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn select_list_layout_uses_bounded_primary_columns() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<LayoutInput, Rows>("select_list_layout_uses_bounded_primary_columns")
    {
        let (_, theme) = published::published(&colors(0));
        let (_scratch, state) = registry(&["dark".into(), "light".into(), "a".repeat(50)]);
        let component: Rc<dyn Component> = match query.kind {
            Kind::Thinking => Rc::new(ThinkingSelectorComponent::new(
                ThinkingLevel::Off,
                levels(),
                Rc::new(|_| {}),
                Rc::new(|| {}),
                theme,
            )),
            Kind::Theme => Rc::new(
                maestro_chat::ThemeSelectorComponent::new(
                    "dark",
                    Rc::new(|_| {}),
                    Rc::new(|| {}),
                    Rc::new(|_| {}),
                    &state,
                )
                .unwrap(),
            ),
            Kind::Images => Rc::new(maestro_chat::ShowImagesSelectorComponent::new(
                false,
                Rc::new(|_| {}),
                Rc::new(|| {}),
                theme,
            )),
            Kind::Messages => unreachable!(),
        };
        assert_eq!(component.render(query.width), expected.lines);
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn theme_preview_follows_navigation_not_programmatic_changes() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<Operations, Snapshots>("theme_preview_follows_navigation_not_programmatic_changes")
    {
        let (_scratch, state) = registry(&[]);
        let events = Events::default();
        let selector = maestro_chat::ThemeSelectorComponent::new(
            "light",
            events.select(),
            events.cancel(),
            events.preview(),
            &state,
        )
        .unwrap();
        let list = selector.get_select_list();
        let mut frames = vec![snapshot(&selector, list, &events, 80)];
        for operation in query.ops {
            match operation {
                Operation::Index { index } => list.set_selected_index(index),
                Operation::Filter { filter } => list.set_filter(&filter),
                Operation::Key { key } => input(&**list, &[&key]),
            }
            frames.push(snapshot(&selector, list, &events, 80));
        }
        assert_eq!(frames, expected.frames);
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn theme_window_shows_ten_names_with_scroll_position() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<ThemeWindow, Rows>("theme_window_shows_ten_names_with_scroll_position")
    {
        let (_scratch, state) = registry(&query.names);
        let selector = maestro_chat::ThemeSelectorComponent::new(
            &query.names[query.current_index],
            Rc::new(|_| {}),
            Rc::new(|| {}),
            Rc::new(|_| {}),
            &state,
        )
        .unwrap();
        assert_eq!(selector.render(80), expected.lines);
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn theme_discovery_failure_propagates_without_callbacks() {
    let _bindings = bindings_guard();
    for (_, expected) in
        cases::<Empty, Failure>("theme_discovery_failure_propagates_without_callbacks")
    {
        let state = published::unpublished();
        let events = Events::default();
        let result = maestro_chat::ThemeSelectorComponent::new(
            "dark",
            events.select(),
            events.cancel(),
            events.preview(),
            &state,
        );
        assert_eq!(result.is_err(), expected.failed);
        let error = result.err().unwrap();
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind()
                == std::io::ErrorKind::NotFound,
            expected.missing_file
        );
        assert_eq!(
            events.get(),
            expected
                .events
                .into_iter()
                .map(|value| vec![serde_json::Value::from(value)])
                .collect::<Vec<_>>()
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_initial_identity_defaults_to_newest() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<MessageInitial, Frame>("message_initial_identity_defaults_to_newest")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let runtime = manual_runtime::ManualRuntime::new();
        let messages = messages(query.messages);
        let selector = maestro_chat::UserMessageSelectorComponent::new(
            messages,
            events.select(),
            events.cancel(),
            maestro_chat::components::user_message_selector::UserMessageSelectorOptions {
                initial_selected_id: query.initial,
                theme,
                runtime: runtime.handle(),
            },
        );
        input(&**selector.get_message_list(), &["\r"]);
        assert_eq!(
            Frame {
                lines: selector.render(80),
                events: events.get()
            },
            expected
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_initial_duplicate_uses_first_and_empty_id_is_absent() {
    let _bindings = bindings_guard();
    for (query, expected) in cases::<MessageInitial, Frame>(
        "message_initial_duplicate_uses_first_and_empty_id_is_absent",
    ) {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = message_selector(
            messages(query.messages),
            query.initial,
            &events,
            theme,
            &manual_runtime::ManualRuntime::new(),
        );
        input(&**selector.get_message_list(), &["\r"]);
        assert_eq!(
            Frame {
                lines: selector.render(80),
                events: events.get()
            },
            expected
        );
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_text_replaces_only_lf_and_trims_defined_whitespace() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<TextInput, Rows>("message_text_replaces_only_lf_and_trims_defined_whitespace")
    {
        let (_, theme) = published::published(&colors(0));
        let records = vec![
            maestro_chat::components::user_message_selector::UserMessageItem {
                id: "a".into(),
                text: query.text,
                timestamp: None,
            },
        ];
        let selector = message_selector(
            records,
            None,
            &Events::default(),
            theme,
            &manual_runtime::ManualRuntime::new(),
        );
        assert_eq!(selector.get_message_list().render(80), expected.lines);
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_clipping_preserves_cells_graphemes_and_escape_sequences() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<ClipInput, Rows>("message_clipping_preserves_cells_graphemes_and_escape_sequences")
    {
        let (_, theme) = published::published(&colors(0));
        let records = vec![
            maestro_chat::components::user_message_selector::UserMessageItem {
                id: "a".into(),
                text: query.text,
                timestamp: None,
            },
        ];
        let selector = message_selector(
            records,
            None,
            &Events::default(),
            theme,
            &manual_runtime::ManualRuntime::new(),
        );
        assert_eq!(
            selector.get_message_list().render(query.width),
            expected.lines
        );
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_window_centers_and_clamps_ten_rows() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<MessageWindow, Rows>("message_window_centers_and_clamps_ten_rows")
    {
        let (_, theme) = published::published(&colors(0));
        let initial = query.messages[query.index].id.clone();
        let selector = message_selector(
            messages(query.messages),
            Some(initial),
            &Events::default(),
            theme,
            &manual_runtime::ManualRuntime::new(),
        );
        assert_eq!(selector.get_message_list().render(80), expected.lines);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_navigation_wraps_and_empty_confirmation_is_inert() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<Navigation, Frames>("message_navigation_wraps_and_empty_confirmation_is_inert")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = message_selector(
            entries(query.n),
            None,
            &events,
            theme,
            &manual_runtime::ManualRuntime::new(),
        );
        let list = selector.get_message_list();
        let mut frames = Vec::new();
        for key in query.ops {
            input(&**list, &[&key]);
            frames.push(Frame {
                lines: list.render(80),
                events: events.get(),
            });
        }
        assert_eq!(frames, expected.frames);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_binding_collision_prefers_navigation() {
    let _bindings = bindings_guard();
    for (_, expected) in cases::<Empty, Frame>("message_binding_collision_prefers_navigation") {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = message_selector(
            entries(3),
            None,
            &events,
            theme,
            &manual_runtime::ManualRuntime::new(),
        );
        bindings(["x", "x", "x", "x"]);
        input(&**selector.get_message_list(), &["x"]);
        maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
            maestro_tui::TUI_KEYBINDINGS.clone(),
            vec![],
        ));
        input(&**selector.get_message_list(), &["\r"]);
        assert_eq!(
            Frame {
                lines: selector.get_message_list().render(80),
                events: events.get()
            },
            expected
        );
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn message_callbacks_are_replaceable_clearable_and_reentrant() {
    let _bindings = bindings_guard();
    for (_, expected) in
        cases::<Empty, Frame>("message_callbacks_are_replaceable_clearable_and_reentrant")
    {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let selector = message_selector(
            entries(3),
            None,
            &events,
            theme,
            &manual_runtime::ManualRuntime::new(),
        );
        let list = selector.get_message_list();
        list.set_on_select(None);
        list.set_on_cancel(None);
        assert!(list.on_select().is_none());
        assert!(list.on_cancel().is_none());
        input(&**list, &["\r", "\x1b"]);
        let weak = Rc::downgrade(list);
        let transcript = events.clone();
        let select: Rc<dyn Fn(&str)> = Rc::new(move |id| {
            transcript.push("new-select", Some(id.into()));
            let list = weak.upgrade().unwrap();
            list.set_on_select(None);
            input(&*list, &["\x1b[A"]);
        });
        let weak = Rc::downgrade(list);
        let transcript = events.clone();
        let cancel: Rc<dyn Fn()> = Rc::new(move || {
            transcript.push("new-cancel", None);
            weak.upgrade().unwrap().set_on_cancel(None);
        });
        list.set_on_select(Some(select.clone()));
        list.set_on_cancel(Some(cancel.clone()));
        assert!(Rc::ptr_eq(&list.on_select().unwrap(), &select));
        assert!(Rc::ptr_eq(&list.on_cancel().unwrap(), &cancel));
        input(&**list, &["\r", "\r", "\x1b", "\x1b"]);
        assert_eq!(
            Frame {
                lines: list.render(80),
                events: events.get()
            },
            expected
        );
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn selectors_read_live_bindings_and_reject_wrong_modifiers() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<KindInput, Frames>("selectors_read_live_bindings_and_reject_wrong_modifiers")
    {
        let (_scratch, state) = registry(&[]);
        let events = Events::default();
        let selector = picker(
            query.kind,
            3,
            &state,
            &events,
            &manual_runtime::ManualRuntime::new(),
        );
        bindings(["k", "j", "x", "q"]);
        let list = selector.list();
        let mut frames = Vec::new();
        for key in [
            "\x1b[A",
            "\x1b[B",
            "\r",
            "\x1b",
            "\x1b[120;2u",
            "j",
            "x",
            "k",
            "q",
        ] {
            input(&*list, &[key]);
            frames.push(Frame {
                lines: list.render(80),
                events: events.get(),
            });
        }
        assert_eq!(frames, expected.frames);
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn selector_live_colors_preserve_construction_time_help() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<KindInput, ColorFrames>("selector_live_colors_preserve_construction_time_help")
    {
        let (_scratch, state) = registry(&[]);
        let selector = picker(
            query.kind,
            1,
            &state,
            &Events::default(),
            &manual_runtime::ManualRuntime::new(),
        );
        assert_eq!(selector.container().render(80), expected.before);
        state.set_theme_instance(colors(4)).unwrap();
        selector.container().invalidate();
        assert_eq!(selector.container().render(80), expected.after);
    }
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn selector_children_and_focus_list_keep_distinct_identities() {
    let _bindings = bindings_guard();
    for (query, expected) in
        cases::<KindInput, Children>("selector_children_and_focus_list_keep_distinct_identities")
    {
        let (_scratch, state) = registry(&[]);
        let events = Events::default();
        let selector = picker(
            query.kind,
            1,
            &state,
            &events,
            &manual_runtime::ManualRuntime::new(),
        );
        state.set_theme_instance(colors(4)).unwrap();
        let list = selector.list();
        assert_eq!(Rc::ptr_eq(&list, &selector.list()), expected.identity);
        let children = selector.container().children();
        selector.container().clear();
        assert!(!Rc::ptr_eq(&children, &selector.container().children()));
        assert_eq!(selector.container().render(80), expected.cleared);
        assert_eq!(list.render(80), expected.retained);
        input(&*list, &["\r"]);
        assert_eq!(events.get().len(), 1);
        selector.container().set_children(children);
        assert_eq!(selector.container().render(80), expected.restored);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn empty_message_auto_cancel_uses_one_original_delayed_callback() {
    let _bindings = bindings_guard();
    for (query, expected) in cases::<TimerInput, TimerOutput>(
        "empty_message_auto_cancel_uses_one_original_delayed_callback",
    ) {
        let (_, theme) = published::published(&colors(0));
        let events = Events::default();
        let transcript = events.clone();
        let on_cancel: Rc<dyn Fn()> = Rc::new(move || transcript.push("original-cancel", None));
        let runtime = manual_runtime::ManualRuntime::new();
        let count = usize::from(matches!(query.mode, TimerMode::Nonempty));
        let selector = maestro_chat::UserMessageSelectorComponent::new(
            entries(count),
            events.select(),
            on_cancel,
            maestro_chat::components::user_message_selector::UserMessageSelectorOptions {
                initial_selected_id: None,
                theme,
                runtime: runtime.handle(),
            },
        );
        assert_eq!(selector.render(80), expected.initial.lines);
        assert_eq!(events.get(), expected.initial.events);
        assert_eq!(runtime.pending(), expected.initial.delays.len());
        let delays: Vec<_> = runtime
            .next_deadline()
            .map(|deadline| u64::try_from(deadline.as_millis()).unwrap())
            .into_iter()
            .collect();
        assert_eq!(delays, expected.initial.delays);
        match query.mode {
            TimerMode::CancelBeforeTimer => input(&**selector.get_message_list(), &["\x1b"]),
            TimerMode::ReplaceCallback => {
                let transcript = events.clone();
                selector
                    .get_message_list()
                    .set_on_cancel(Some(Rc::new(move || {
                        transcript.push("replacement-cancel", None);
                    })));
            }
            TimerMode::ParentRemoved => selector.clear(),
            TimerMode::Empty | TimerMode::Nonempty => {}
        }
        advance(&runtime, 99);
        assert_eq!(events.get(), expected.before);
        advance(&runtime, 100);
        assert_eq!(events.get(), expected.at);
        advance(&runtime, 200);
        assert_eq!(events.get(), expected.after);
        assert_eq!(runtime.pending(), expected.pending);
    }
}

#[cfg(not(target_arch = "wasm32"))]
use crate::recording_terminal;
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn selector_focus_handles_outlive_chrome_without_retaining_it() {
    let _bindings = bindings_guard();
    for kind in [Kind::Thinking, Kind::Theme, Kind::Images, Kind::Messages] {
        let (_scratch, state) = registry(&[]);
        let events = Events::default();
        let runtime = manual_runtime::ManualRuntime::new();
        let selector = Rc::new(picker(kind, 3, &state, &events, &runtime));
        let parent = Rc::downgrade(&selector);
        let list = selector.list();
        let child = Rc::downgrade(&list);
        let terminal = recording_terminal::RecordingTerminal::new(80, 24);
        let tui = maestro_tui::TUI::new(
            terminal.handle(),
            runtime.handle(),
            maestro_tui::TerminalImage::new(|_| None, || 1),
            Some(false),
        );
        tui.set_focus(Some(list.clone()));
        drop(selector);
        assert!(parent.upgrade().is_none());
        tui.start().unwrap();
        terminal.send_input("\r");
        let expected = match kind {
            Kind::Thinking => serde_json::json!([["select", "low"]]),
            Kind::Theme => serde_json::json!([["select", "dark"]]),
            Kind::Images => serde_json::json!([["select", true]]),
            Kind::Messages => serde_json::json!([["select", "id-2"]]),
        };
        assert_eq!(serde_json::to_value(events.get()).unwrap(), expected);
        tui.set_focus(None);
        tui.stop().unwrap();
        drop(tui);
        drop(list);
        assert!(child.upgrade().is_none());
        runtime.settle().unwrap();
    }
    empty_callback_releases_captures();
}

/// The scheduled closure retains only its original callback until settlement.
#[cfg(not(target_arch = "wasm32"))]
fn empty_callback_releases_captures() {
    let runtime = manual_runtime::ManualRuntime::new();
    let (_, theme) = published::published(&colors(0));
    let capture = Rc::new(std::cell::Cell::new(0));
    let weak_capture = Rc::downgrade(&capture);
    let callback: Rc<dyn Fn()> = Rc::new(move || capture.set(capture.get() + 1));
    let weak_callback = Rc::downgrade(&callback);
    let selector = Rc::new(maestro_chat::UserMessageSelectorComponent::new(
        vec![],
        Rc::new(|_| {}),
        callback,
        maestro_chat::components::user_message_selector::UserMessageSelectorOptions {
            initial_selected_id: None,
            theme,
            runtime: runtime.handle(),
        },
    ));
    let weak_parent = Rc::downgrade(&selector);
    let weak_list = Rc::downgrade(selector.get_message_list());
    drop(selector);
    assert!(weak_parent.upgrade().is_none());
    assert!(weak_list.upgrade().is_none());
    assert!(weak_callback.upgrade().is_some());
    assert_eq!(runtime.pending(), 1);
    assert_eq!(runtime.advance_to(manual_runtime::ms(100)).len(), 1);
    assert!(weak_callback.upgrade().is_none());
    assert!(weak_capture.upgrade().is_none());
    assert_eq!(runtime.pending(), 0);
}

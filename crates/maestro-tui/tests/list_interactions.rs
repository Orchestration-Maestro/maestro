#![cfg(test)]
#[cfg(test)]
pub mod list_support;
use maestro_tui::{
    Component, KeybindingKeys, KeybindingsManager, SelectItem, SelectList, SelectListLayoutOptions,
    SettingItem, SettingsList, TUI_KEYBINDINGS, set_keybindings,
};
use maestro_tui::{components::settings_list::SettingsListOptions, tui::InputHandler};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};

#[test]
fn list_visible_limits_keep_scroll_counters() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("list_visible_limits_keep_scroll_counters") {
        if case.case.starts_with("sl-") {
            list_support::select_case(case);
        } else {
            list_support::settings_case(case);
        }
    }
}
#[test]
fn lists_clip_empty_messages_and_prefixes() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("lists_clip_empty_messages_and_prefixes") {
        if case.case.starts_with("sl-") {
            list_support::select_case(case);
        } else {
            list_support::settings_case(case);
        }
    }
}
#[test]
fn lists_respect_rebound_key_precedence() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("lists_respect_rebound_key_precedence") {
        let actual = match case.case.as_str() {
            "keys-rebound" => rebound_keys(),
            "list-capabilities" => {
                let calls = Rc::default();
                let a = SelectList::new(
                    Vec::new(),
                    1,
                    list_support::select_theme(false, &calls),
                    SelectListLayoutOptions::default(),
                );
                let b = SettingsList::new(
                    Vec::new(),
                    1,
                    list_support::settings_theme(false, &calls),
                    (Rc::new(|_, _| {}), Rc::new(|| {})),
                    SettingsListOptions::default(),
                );
                json!({"select":[a.input_handler().is_some(),a.focusable().is_some()],"settings":[b.input_handler().is_some(),b.focusable().is_some()]})
            }
            other => panic!("unknown case {other}"),
        };
        let id = case.case.clone();
        list_support::special(case, &id, &actual);
    }
}
#[test]
fn lists_theme_closures_read_live_captured_state() {
    let _keys = list_support::KEYS.lock().unwrap();
    let prefix = Rc::new(RefCell::new("a:"));
    let calls = Rc::default();
    let mut select_theme = list_support::select_theme(false, &calls);
    let captured = Rc::clone(&prefix);
    select_theme.selected_text = Rc::new(move |text| format!("{}{text}", *captured.borrow()));
    let a = SelectList::new(
        vec![
            SelectItem {
                value: "x".into(),
                label: "X".into(),
                description: None,
            },
            SelectItem {
                value: "y".into(),
                label: "Y".into(),
                description: None,
            },
        ],
        2,
        select_theme,
        SelectListLayoutOptions::default(),
    );
    a.set_selected_index(1);
    let mut settings_theme = list_support::settings_theme(false, &calls);
    let captured = Rc::clone(&prefix);
    settings_theme.value = Rc::new(move |text, _| format!("{}{text}", *captured.borrow()));
    let b = SettingsList::new(
        [("x", "X", "V"), ("y", "XY", "W"), ("z", "Z", "Q")]
            .into_iter()
            .map(|(id, label, value)| SettingItem {
                id: id.into(),
                label: label.into(),
                current_value: value.into(),
                description: None,
                values: None,
                submenu: None,
            })
            .collect(),
        3,
        settings_theme,
        (Rc::new(|_, _| {}), Rc::new(|| {})),
        SettingsListOptions {
            enable_search: true,
        },
    );
    b.handle_input("X");
    b.handle_input("\x1b[B");
    let before = [a.render(40), b.render(40)];
    prefix.replace("b:");
    a.invalidate();
    b.invalidate();
    list_support::special(
        list_support::cases("lists_theme_closures_read_live_captured_state").remove(0),
        "themes-live",
        &json!({"before":before,"after":[a.render(40),b.render(40)]}),
    );
}

/// Executes the saved keys-rebound callback scenario.
fn rebound_keys() -> serde_json::Value {
    let config = [
        ("tui.select.up", vec!["x"]),
        ("tui.select.down", vec!["x", "y"]),
        ("tui.select.confirm", vec!["z"]),
        ("tui.select.cancel", vec!["q"]),
    ]
    .into_iter()
    .map(|(action, keys)| {
        (
            action.into(),
            Some(KeybindingKeys::Multiple(
                keys.into_iter().map(str::to_owned).collect(),
            )),
        )
    })
    .collect();
    set_keybindings(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), config));
    let events: list_support::Observations = Rc::default();
    let calls = Rc::default();
    let a = SelectList::new(
        list_support::commands(),
        3,
        list_support::select_theme(false, &calls),
        SelectListLayoutOptions::default(),
    );
    let log = Rc::clone(&events);
    a.set_on_select(Some(Rc::new(move |item| {
        log.borrow_mut().push(json!(["select", item.value]));
    })));
    let log = Rc::clone(&events);
    a.set_on_cancel(Some(Rc::new(move || {
        log.borrow_mut().push(json!(["cancel"]));
    })));
    for key in ["\x1b[B", "x", "z", "q"] {
        a.handle_input(key);
    }
    let log = Rc::clone(&events);
    let cancel = Rc::clone(&events);
    let b = SettingsList::new(
        list_support::settings(),
        3,
        list_support::settings_theme(false, &calls),
        (
            Rc::new(move |id, value| {
                log.borrow_mut().push(json!(["change", id, value]));
            }),
            Rc::new(move || cancel.borrow_mut().push(json!(["cancel-setting"]))),
        ),
        SettingsListOptions {
            enable_search: true,
        },
    );
    for key in ["x", "z", "q"] {
        b.handle_input(key);
    }
    let result = json!({"select":a.get_selected_item().unwrap().value,"settings":b.render(80),"events":*events.borrow()});
    set_keybindings(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Vec::new()));
    result
}

#[test]
fn lists_preserve_fitting_theme_bytes() {
    let _keys = list_support::KEYS.lock().unwrap();
    for output in ["\x1b[31m", "\x1b]8;;https://example.test\x07row"] {
        let mut theme = list_support::select_theme(false, &Rc::default());
        theme.selected_text = Rc::new(move |_| output.into());
        theme.no_match = Rc::new(move |_| output.into());
        let list = SelectList::new(
            vec![SelectItem {
                value: "command".into(),
                label: "Command".into(),
                description: None,
            }],
            1,
            theme,
            SelectListLayoutOptions::default(),
        );
        assert_eq!(list.render(80), [output]);
        assert_eq!(list.render(0), [""]);
        list.set_filter("missing");
        assert_eq!(list.render(80), [output]);
        assert_eq!(list.render(0), [""]);
        let mut theme = list_support::settings_theme(false, &Rc::default());
        theme.hint = Rc::new(move |_| output.into());
        let list = SettingsList::new(
            vec![],
            1,
            theme,
            (Rc::new(|_, _| {}), Rc::new(|| {})),
            SettingsListOptions::default(),
        );
        assert_eq!(list.render(80), [output]);
        assert_eq!(list.render(0), [""]);
        let mut theme = list_support::settings_theme(false, &Rc::default());
        theme.description = Rc::new(move |_| output.into());
        let list = SettingsList::new(
            vec![SettingItem {
                id: "x".into(),
                label: "X".into(),
                description: Some("description".into()),
                current_value: "V".into(),
                values: None,
                submenu: None,
            }],
            1,
            theme,
            (Rc::new(|_, _| {}), Rc::new(|| {})),
            SettingsListOptions::default(),
        );
        assert_eq!(list.render(80)[2], output);
        assert_eq!(list.render(0)[2], "");
    }
}

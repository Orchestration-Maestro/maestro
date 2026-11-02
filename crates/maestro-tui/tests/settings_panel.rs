#![cfg(test)]
#[cfg(test)]
use crate::list_support;

#[test]
fn settings_empty_states_keep_hint_order() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_empty_states_keep_hint_order") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_search_filters_labels_and_resets() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_search_filters_labels_and_resets") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_spaces_activate_or_are_removed() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_spaces_activate_or_are_removed") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_search_reuses_input_decoding() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_search_reuses_input_decoding") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_search_retains_cursor_and_edit_history() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_search_retains_cursor_and_edit_history") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_cycle_handles_absent_duplicate_and_empty_values() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_cycle_handles_absent_duplicate_and_empty_values") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_updates_first_id_without_change_callback() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_updates_first_id_without_change_callback") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_navigation_wraps_and_keeps_selection() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_navigation_wraps_and_keeps_selection") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_layout_clips_after_styling() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_layout_clips_after_styling") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_alignment_uses_all_original_labels() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_alignment_uses_all_original_labels") {
        list_support::settings_case(case);
    }
}

#[test]
fn settings_search_reuses_fuzzy_ranking() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_search_reuses_fuzzy_ranking") {
        list_support::settings_case(case);
    }
}

use maestro_tui::components::settings_list::SettingsListOptions;
use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, SettingItem, SettingsList};
use serde_json::json;
use std::{
    cell::{OnceCell, RefCell},
    rc::{Rc, Weak},
};
type Done = Rc<dyn Fn(Option<String>)>;
fn item() -> SettingItem {
    SettingItem {
        id: "x".into(),
        label: "X".into(),
        current_value: "old".into(),
        description: None,
        values: None,
        submenu: None,
    }
}
fn child() -> Rc<dyn Component> {
    Rc::new(list_support::Child {
        text: "child",
        events: None,
        accepts_input: false,
    })
}
fn theme() -> maestro_tui::SettingsListTheme {
    list_support::settings_theme(false, &Rc::default())
}
#[test]
fn settings_submenu_delegates_and_done_restores() {
    let _keys = list_support::KEYS.lock().unwrap();
    let done: Rc<RefCell<Option<Done>>> = Rc::default();
    let events: list_support::Observations = Rc::default();
    let owner: Rc<OnceCell<Weak<SettingsList>>> = Rc::default();
    let mut items = list_support::settings();
    let saved = Rc::clone(&done);
    let log = Rc::clone(&events);
    let target = Rc::clone(&owner);
    items[1].submenu = Some(Rc::new(move |value, callback| {
        log.borrow_mut().push(json!(["open", value]));
        saved.replace(Some(callback));
        target
            .get()
            .unwrap()
            .upgrade()
            .unwrap()
            .handle_input("\x1b[B");
        Rc::new(list_support::Child {
            text: "child",
            events: Some(Rc::clone(&log)),
            accepts_input: true,
        })
    }));
    let log = Rc::clone(&events);
    let target = Rc::clone(&owner);
    let cancel = Rc::clone(&events);
    let list = Rc::new(SettingsList::new(
        items,
        3,
        theme(),
        (
            Rc::new(move |id, value| {
                let frame = target.get().unwrap().upgrade().unwrap().render(80);
                log.borrow_mut().push(json!(["change", id, value, frame]));
                true
            }),
            Rc::new(move || cancel.borrow_mut().push(json!(["cancel"]))),
        ),
        SettingsListOptions::default(),
    ));
    assert!(owner.set(Rc::downgrade(&list)).is_ok());
    list.handle_input("\x1b[B");
    list.handle_input(" ");
    let a = list.render(20);
    list.invalidate();
    list.handle_input("\x1b");
    let callback = done.borrow().clone().unwrap();
    callback(Some(String::new()));
    let b = list.render(80);
    list.handle_input("\r");
    let callback = done.borrow().clone().unwrap();
    callback(None);
    let c = list.render(80);
    callback(Some("late".into()));
    let final_frame = list.render(80);
    list_support::special(
        list_support::cases("settings_submenu_delegates_and_done_restores").remove(0),
        "st-submenu",
        &json!({"frames":[a,b,c],"events":*events.borrow(),"final":final_frame}),
    );
}
#[test]
fn settings_synchronous_done_does_not_reopen_submenu() {
    let _keys = list_support::KEYS.lock().unwrap();
    let events: list_support::Observations = Rc::default();
    let mut setting = item();
    setting.submenu = Some(Rc::new(|_, done| {
        done(Some("new".into()));
        Rc::new(list_support::Child {
            text: "stale child",
            events: None,
            accepts_input: false,
        })
    }));
    let log = Rc::clone(&events);
    let list = SettingsList::new(
        vec![setting],
        1,
        theme(),
        (
            Rc::new(move |id, value| record_change(&log, id, value)),
            Rc::new(|| {}),
        ),
        SettingsListOptions::default(),
    );
    list.handle_input("\r");
    list_support::special(
        list_support::cases("settings_synchronous_done_does_not_reopen_submenu").remove(0),
        "st-sync-done",
        &json!({"lines":list.render(80),"events":*events.borrow()}),
    );
}
#[test]
fn settings_submenu_optional_input_is_a_noop() {
    let _keys = list_support::KEYS.lock().unwrap();
    let cancels = Rc::new(std::cell::Cell::new(0));
    let mut setting = item();
    setting.current_value = "x".into();
    setting.submenu = Some(Rc::new(|_, _| {
        Rc::new(list_support::Child {
            text: "only",
            events: None,
            accepts_input: false,
        })
    }));
    let count = Rc::clone(&cancels);
    let list = SettingsList::new(
        vec![setting],
        1,
        theme(),
        (
            Rc::new(|_, _| true),
            Rc::new(move || count.set(count.get() + 1)),
        ),
        SettingsListOptions::default(),
    );
    list.handle_input("\r");
    list.handle_input("\x1b");
    list.invalidate();
    list_support::special(
        list_support::cases("settings_submenu_optional_input_is_a_noop").remove(0),
        "st-child-no-input",
        &json!({"lines":list.render(10),"cancels":cancels.get()}),
    );
}
#[test]
fn settings_change_callback_can_reenter() {
    let _keys = list_support::KEYS.lock().unwrap();
    let owner: Rc<OnceCell<Weak<SettingsList>>> = Rc::default();
    let events: list_support::Observations = Rc::default();
    let log = Rc::clone(&events);
    let target = Rc::clone(&owner);
    let list = Rc::new(SettingsList::new(
        list_support::settings(),
        3,
        theme(),
        (
            Rc::new(move |id, value| {
                let list = target.get().unwrap().upgrade().unwrap();
                let frame = list.render(80);
                log.borrow_mut().push(json!([id, value, frame]));
                list.update_value(id, "callback".into());
                list.handle_input("\x1b[B");
                true
            }),
            Rc::new(|| {}),
        ),
        SettingsListOptions::default(),
    ));
    assert!(owner.set(Rc::downgrade(&list)).is_ok());
    list.handle_input("\r");
    list_support::special(
        list_support::cases("settings_change_callback_can_reenter").remove(0),
        "st-change-reentry",
        &json!({"events":*events.borrow(),"lines":list.render(80)}),
    );
}
#[test]
fn settings_render_reads_updates_through_original_items() {
    let _keys = list_support::KEYS.lock().unwrap();
    let owner: Rc<OnceCell<Weak<SettingsList>>> = Rc::default();
    let mut style = theme();
    let target = Rc::clone(&owner);
    let once = std::cell::Cell::new(true);
    style.label = Rc::new(move |text, _| {
        if once.replace(false) {
            let list = target.get().unwrap().upgrade().unwrap();
            list.update_value("a", "changed".into());
            list.update_value("c", "later".into());
            list.handle_input("gm");
        }
        text.into()
    });
    let list = Rc::new(SettingsList::new(
        list_support::settings(),
        3,
        style,
        (Rc::new(|_, _| true), Rc::new(|| {})),
        SettingsListOptions {
            enable_search: true,
        },
    ));
    assert!(owner.set(Rc::downgrade(&list)).is_ok());
    list_support::special(
        list_support::cases("settings_render_reads_updates_through_original_items").remove(0),
        "st-internal-alias",
        &json!({"first":list.render(80),"second":list.render(80)}),
    );
}
#[test]
fn settings_done_updates_captured_item_after_filter_changes() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("settings_done_updates_captured_item_after_filter_changes") {
        let done: Rc<RefCell<Option<Done>>> = Rc::default();
        let events: list_support::Observations = Rc::default();
        let saved = Rc::clone(&done);
        let log = Rc::clone(&events);
        let factory = Rc::new(move |_: String, callback: Done| {
            saved.replace(Some(callback));
            child()
        });
        let actual = match case.case.as_str() {
            "st-done-filter" => {
                let mut items = list_support::settings();
                items[0].submenu = Some(factory);
                let list = SettingsList::new(
                    items,
                    3,
                    theme(),
                    (
                        Rc::new(move |id, value| record_change(&log, id, value)),
                        Rc::new(|| {}),
                    ),
                    SettingsListOptions {
                        enable_search: true,
                    },
                );
                list.handle_input("\r");
                let callback = done.borrow().clone().unwrap();
                callback(None);
                list.handle_input("a");
                list.handle_input("\x1b[B");
                callback(Some("retained".into()));
                list.handle_input("\x7f");
                json!({"events":*events.borrow(),"lines":list.render(80)})
            }
            "st-detached-done" => {
                let mut setting = item();
                setting.submenu = Some(factory);
                let list = SettingsList::new(
                    vec![setting],
                    1,
                    theme(),
                    (
                        Rc::new(move |id, value| record_change(&log, id, value)),
                        Rc::new(|| {}),
                    ),
                    SettingsListOptions::default(),
                );
                list.handle_input("\r");
                drop(list);
                let callback = done.borrow().clone().unwrap();
                callback(Some("later".into()));
                callback(Some("again".into()));
                json!({"events":*events.borrow()})
            }
            other => panic!("unknown case {other}"),
        };
        let id = case.case.clone();
        list_support::special(case, &id, &actual);
    }
}
#[test]
fn settings_done_closes_after_reentrant_change() {
    let _keys = list_support::KEYS.lock().unwrap();
    let done: Rc<RefCell<Option<Done>>> = Rc::default();
    let events: list_support::Observations = Rc::default();
    let owner: Rc<OnceCell<Weak<SettingsList>>> = Rc::default();
    let mut items = list_support::settings();
    let saved = Rc::clone(&done);
    items[0].submenu = Some(Rc::new(move |_, callback| {
        saved.replace(Some(callback));
        child()
    }));
    let target = Rc::clone(&owner);
    let log = Rc::clone(&events);
    let list = Rc::new(SettingsList::new(
        items,
        3,
        theme(),
        (
            Rc::new(move |_, _| {
                let list = target.get().unwrap().upgrade().unwrap();
                let frame = list.render(20);
                log.borrow_mut().push(json!(frame));
                list.update_value("a", "override".into());
                true
            }),
            Rc::new(|| {}),
        ),
        SettingsListOptions::default(),
    ));
    assert!(owner.set(Rc::downgrade(&list)).is_ok());
    list.handle_input("\r");
    let callback = done.borrow().clone().unwrap();
    callback(Some("new".into()));
    list_support::special(
        list_support::cases("settings_done_closes_after_reentrant_change").remove(0),
        "st-done-order",
        &json!({"events":*events.borrow(),"lines":list.render(80)}),
    );
}

#[test]
fn settings_submenu_returns_child_input_failure() {
    use crate::fallible_input::{Receiver, cases, error_value};
    let _keys = list_support::KEYS.lock().unwrap();
    for case in cases("settings_submenu_returns_child_input_failure") {
        assert!(case.input.gate.is_none() && case.input.data.is_none());
        let child = case.input.child.unwrap();
        let receiver = Rc::new(Receiver {
            events: Rc::default(),
            failing: child == "failure",
            accepts: child != "absent-handler",
            releases: false,
            identity: Rc::new(()),
        });
        let mut setting = item();
        let retained = receiver.clone();
        setting.submenu = Some(Rc::new(move |_, _| retained.clone()));
        let list = SettingsList::new(
            vec![setting],
            1,
            theme(),
            (
                Rc::new(|_, _| panic!("parent must not activate")),
                Rc::new(|| panic!("parent must not cancel")),
            ),
            SettingsListOptions::default(),
        );
        list.handle_input("\r");
        let result = list.try_handle_input("x");
        let calls: Vec<_> = receiver
            .events
            .borrow()
            .iter()
            .map(|event| {
                assert_eq!(event[0], "input");
                event[1].clone()
            })
            .collect();
        let actual = match result {
            Ok(()) => json!({"branch":"ok","value":calls,"calls":calls}),
            Err(error) => {
                json!({"branch":"error","error":error_value(error.as_ref(),&receiver.identity),"calls":calls})
            }
        };
        assert!(actual == case.expected, "recorded submenu delegation");
    }
}

#[test]
fn refused_settings_changes_restore_cycle_and_submenu_values() {
    let _keys = list_support::KEYS.lock().unwrap();
    for nested in [false, true] {
        let mut setting = item();
        if nested {
            setting.submenu = Some(Rc::new(|_, done| {
                done(Some("new".into()));
                child()
            }));
        } else {
            setting.values = Some(vec!["old".into(), "new".into()]);
        }
        let list = SettingsList::new(
            vec![setting],
            1,
            theme(),
            (Rc::new(|_, _| false), Rc::new(|| {})),
            SettingsListOptions::default(),
        );
        let before = list.render(80);
        list.handle_input("\r");
        assert_eq!(list.render(80), before, "nested: {nested}");
    }
}

fn record_change(log: &list_support::Observations, id: &str, value: &str) -> bool {
    log.borrow_mut().push(json!([id, value]));
    true
}

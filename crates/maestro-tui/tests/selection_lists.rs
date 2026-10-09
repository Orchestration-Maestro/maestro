#![cfg(test)]
#[cfg(test)]
pub mod list_support;

#[test]
fn select_descriptions_collapse_breaks() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_descriptions_collapse_breaks") {
        list_support::select_case(case);
    }
}

#[test]
fn select_columns_keep_alignment() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_columns_keep_alignment") {
        list_support::select_case(case);
    }
}

#[test]
fn select_bounds_choose_minimum() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_bounds_choose_minimum") {
        list_support::select_case(case);
    }
}

#[test]
fn select_bounds_choose_maximum() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_bounds_choose_maximum") {
        list_support::select_case(case);
    }
}

#[test]
fn select_custom_truncation_keeps_context() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_custom_truncation_keeps_context") {
        list_support::select_case(case);
    }
}

#[test]
fn select_descriptions_normalize_before_layout() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_descriptions_normalize_before_layout") {
        list_support::select_case(case);
    }
}

#[test]
fn select_description_thresholds_and_narrow_widths() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_description_thresholds_and_narrow_widths") {
        list_support::select_case(case);
    }
}

#[test]
fn select_bounds_fallback_swap_and_clamp() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_bounds_fallback_swap_and_clamp") {
        list_support::select_case(case);
    }
}

#[test]
fn select_custom_output_is_clipped_twice() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_custom_output_is_clipped_twice") {
        list_support::select_case(case);
    }
}

#[test]
fn select_label_fallback_preserves_value_identity() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_label_fallback_preserves_value_identity") {
        list_support::select_case(case);
    }
}

#[test]
fn select_filter_uses_value_prefix_and_resets() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_filter_uses_value_prefix_and_resets") {
        list_support::select_case(case);
    }
}

#[test]
fn select_case_matching_preserves_unicode() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_case_matching_preserves_unicode") {
        list_support::select_case(case);
    }
}

#[test]
fn select_navigation_wraps_and_centers() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_navigation_wraps_and_centers") {
        list_support::select_case(case);
    }
}

#[test]
fn select_index_clamps_without_notifications() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_index_clamps_without_notifications") {
        list_support::select_case(case);
    }
}

#[test]
fn select_column_measurement_includes_hidden_rows() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_column_measurement_includes_hidden_rows") {
        list_support::select_case(case);
    }
}

use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, SelectItem, SelectList, SelectListLayoutOptions};
use serde_json::json;
use std::{
    cell::{Cell, OnceCell},
    rc::{Rc, Weak},
};
#[test]
fn select_render_reentry_reads_live_filtered_collection() {
    let _keys = list_support::KEYS.lock().unwrap();
    let owner: Rc<OnceCell<Weak<SelectList>>> = Rc::default();
    let once = Cell::new(true);
    let mut theme = list_support::select_theme(false, &Rc::default());
    let slot = Rc::clone(&owner);
    theme.selected_text = Rc::new(move |text| {
        if once.replace(false) {
            slot.get().unwrap().upgrade().unwrap().set_filter("beta");
        }
        text.into()
    });
    let list = Rc::new(SelectList::new(
        list_support::commands(),
        3,
        theme,
        SelectListLayoutOptions::default(),
    ));
    owner.set(Rc::downgrade(&list)).unwrap();
    let actual = json!({"first":list.render(80),"second":list.render(80),"selected":list.get_selected_item().unwrap().value});
    list_support::special(
        list_support::cases("select_render_reentry_reads_live_filtered_collection").remove(0),
        "sl-render-reentry",
        &actual,
    );
}
#[test]
fn select_callbacks_keep_identity_and_reenter() {
    let _keys = list_support::KEYS.lock().unwrap();
    for case in list_support::cases("select_callbacks_keep_identity_and_reenter") {
        let list = Rc::new(SelectList::new(
            list_support::commands(),
            3,
            list_support::select_theme(false, &Rc::default()),
            SelectListLayoutOptions::default(),
        ));
        let events: list_support::Observations = Rc::default();
        let actual = match case.case.as_str() {
            "sl-callbacks" => {
                list.set_selected_index(1);
                let alpine = list.get_selected_item().unwrap();
                list.set_selected_index(0);
                let target = Rc::downgrade(&list);
                let log = Rc::clone(&events);
                let old: Rc<dyn Fn(Rc<SelectItem>)> = Rc::new(move |item| {
                    log.borrow_mut()
                        .push(json!(["old", Rc::ptr_eq(&item, &alpine)]));
                    target.upgrade().unwrap().set_filter("beta");
                });
                list.set_on_selection_change(Some(Rc::clone(&old)));
                list.handle_input("\x1b[B");
                let beta = list.get_selected_item().unwrap();
                let log = Rc::clone(&events);
                list.set_on_selection_change(Some(Rc::new(move |item| {
                    log.borrow_mut()
                        .push(json!(["new", Rc::ptr_eq(&item, &beta)]));
                })));
                list.set_filter("");
                list.set_selected_index(1);
                old(list.get_selected_item().unwrap());
                list.handle_input("\x1b[B");
                json!({"events":*events.borrow(),"selected":list.get_selected_item().unwrap().value})
            }
            "sl-replace-callbacks" => replaced_command_callbacks(&list, &events),
            other => panic!("unknown case {other}"),
        };
        let id = case.case.clone();
        list_support::special(case, &id, &actual);
    }
}
#[test]
fn select_callbacks_and_context_share_selected_item() {
    let _keys = list_support::KEYS.lock().unwrap();
    let selected: Rc<OnceCell<Rc<SelectItem>>> = Rc::default();
    let calls: list_support::Observations = Rc::default();
    let log = Rc::clone(&calls);
    let retained = Rc::clone(&selected);
    let layout = SelectListLayoutOptions {
        truncate_primary: Some(Rc::new(move |context| {
            log.borrow_mut().push(json!([
                "context",
                Rc::ptr_eq(&context.item, retained.get().unwrap())
            ]));
            context.text.into()
        })),
        ..Default::default()
    };
    let list = SelectList::new(
        list_support::commands(),
        3,
        list_support::select_theme(false, &Rc::default()),
        layout,
    );
    assert!(selected.set(list.get_selected_item().unwrap()).is_ok());
    let log = Rc::clone(&calls);
    let item = Rc::clone(selected.get().unwrap());
    list.set_on_select(Some(Rc::new(move |value| {
        log.borrow_mut()
            .push(json!(["selected", Rc::ptr_eq(&value, &item)]));
    })));
    list.render(80);
    list.handle_input("\r");
    list.set_filter("Al");
    list_support::special(
        list_support::cases("select_callbacks_and_context_share_selected_item").remove(0),
        "sl-internal-identity",
        &json!({"calls":*calls.borrow(),"retained":Rc::ptr_eq(&list.get_selected_item().unwrap(),selected.get().unwrap())}),
    );
}

/// Executes the saved sl-replace-callbacks callback scenario.
fn replaced_command_callbacks(
    list: &SelectList,
    events: &list_support::Observations,
) -> serde_json::Value {
    let log = Rc::clone(events);
    list.set_on_select(Some(Rc::new(move |item| {
        log.borrow_mut().push(json!(["old", item.value]));
    })));
    let retained = list.on_select().unwrap();
    let log = Rc::clone(events);
    list.set_on_select(Some(Rc::new(move |item| {
        log.borrow_mut().push(json!(["new", item.value]));
    })));
    list.handle_input("\r");
    list.set_on_select(None);
    list.handle_input("\r");
    retained(list.get_selected_item().unwrap());
    let log = Rc::clone(events);
    list.set_on_cancel(Some(Rc::new(move || {
        log.borrow_mut().push(json!(["cancel"]));
    })));
    let cancel = list.on_cancel().unwrap();
    list.set_on_cancel(None);
    list.handle_input("\x1b");
    cancel();
    list.set_on_selection_change(None);
    list.handle_input("\x1b[B");
    json!({"events":*events.borrow(),"selected":list.get_selected_item().unwrap().value})
}

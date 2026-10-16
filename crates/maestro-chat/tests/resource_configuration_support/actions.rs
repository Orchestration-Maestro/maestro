//! Recorded mutations through the public retained-list and child-array interfaces.
use super::{
    harness::{Harness, colors, item_value},
    records::{Detached, Mutation, Operation},
};
use maestro_chat::components::config_selector::ResourceItem;
use maestro_tui::{
    Component, KeybindingKeys, KeybindingsManager, TUI_KEYBINDINGS, set_keybindings, tui::Focusable,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
impl Harness {
    /// Executes one strictly decoded operation without synthesizing observations.
    pub fn apply(&self, operation: Operation, width: usize) {
        let list = self.component.get_resource_list();
        match operation {
            Operation::Input(data) => list.handle_input(&data).unwrap(),
            Operation::Styles(enabled) => self.styles.set(enabled),
            Operation::Publish(shift) => {
                self.publication.set_theme_instance(colors(shift)).unwrap();
            }
            Operation::Focus(focused) => {
                self.component.focus_flag().set(focused);
                assert_eq!(list.focus_flag().get(), focused);
            }
            Operation::Invalidate => self.component.invalidate(),
            Operation::Bindings(bindings) => set_keybindings(KeybindingsManager::new(
                TUI_KEYBINDINGS.clone(),
                bindings
                    .into_iter()
                    .map(|(key, value)| (key, Some(KeybindingKeys::Single(value))))
                    .collect(),
            )),
            Operation::ClearChildren => {
                self.children.replace(Some(self.component.children()));
                self.component.clear();
            }
            Operation::RestoreChildren => self
                .component
                .set_children(self.children.borrow().as_ref().unwrap().clone()),
            Operation::ReplaceCancelExit => {
                self.replace_close("replacement-close");
                let events = self.events.clone();
                list.set_on_exit(Some(Rc::new(move || {
                    events.borrow_mut().push(json!(["replacement-exit"]));
                })));
                assert!(list.on_exit().is_some());
            }
            Operation::ClearCallbacks => {
                list.set_on_cancel(None);
                list.set_on_exit(None);
                list.set_on_toggle(None);
                assert!(list.on_cancel().is_none());
                assert!(list.on_exit().is_none());
                assert!(list.on_toggle().is_none());
            }
            Operation::InstallObserver => self.observe(width, false),
            Operation::InstallReentrantObserver => self.observe(width, true),
            Operation::MutateCapture(mutation) => self.mutate(mutation),
            Operation::UpdateCapture(enabled) => {
                list.update_item(self.captured.borrow().as_ref().unwrap().clone(), enabled);
            }
            Operation::DetachedUpdate(detached) => self.detached(detached),
            Operation::Reload => {
                let flush = self.settings.borrow().flush();
                self.runtime.block_on(flush);
                self.runtime.block_on(self.settings.borrow_mut().reload());
            }
        }
    }
    /// Replaces the real cancellation slot and verifies the getter's identity.
    fn replace_close(&self, name: &'static str) {
        let events = self.events.clone();
        let callback: Rc<dyn Fn()> = Rc::new(move || events.borrow_mut().push(json!([name])));
        self.component
            .get_resource_list()
            .set_on_cancel(Some(callback.clone()));
        assert!(Rc::ptr_eq(
            &callback,
            &self.component.get_resource_list().on_cancel().unwrap()
        ));
    }
    /// Observes settings and painting inside the callback, then optionally re-enters.
    fn observe(&self, width: usize, reentrant: bool) {
        let list = self.component.get_resource_list();
        let weak = Rc::downgrade(list);
        let settings = self.settings.clone();
        let events = self.events.clone();
        let captured = self.captured.clone();
        list.set_on_toggle(Some(Rc::new(move |item, enabled| {
            captured.replace(Some(item.clone()));
            let list = weak.upgrade().unwrap();
            if reentrant {
                list.set_on_toggle(None);
                list.set_on_cancel(Some(event_callback(events.clone(), "nested-close")));
                list.update_item(item.clone(), true);
            }
            let global = settings.borrow().get_global_settings();
            let project = settings.borrow().get_project_settings();
            let rows = list.try_render(width).unwrap();
            let event = if reentrant {
                json!([
                    "reentry",
                    enabled,
                    item.borrow().enabled,
                    global,
                    project,
                    rows
                ])
            } else {
                json!([
                    "toggle",
                    item_value(&item.borrow()),
                    enabled,
                    global,
                    project,
                    rows
                ])
            };
            events.borrow_mut().push(event);
            if reentrant {
                list.handle_input("\x1b").unwrap();
                list.handle_input(" ").unwrap();
            }
        })));
        assert!(list.on_toggle().is_some());
    }
    /// Mutates every caller-editable field on the actual captured handle.
    fn mutate(&self, value: Mutation) {
        let item = self.captured.borrow().as_ref().unwrap().clone();
        let mut item = item.borrow_mut();
        item.display_name = value.display_name;
        item.path = value.path;
        item.resource_type = value.resource_type;
        item.enabled = value.enabled;
        item.group_key = value.group_key;
        item.subgroup_key = value.subgroup_key;
        item.metadata = value.metadata.native();
    }
    /// Supplies a different object to exercise first-match update semantics.
    fn detached(&self, value: Detached) {
        let original = self.captured.borrow().as_ref().unwrap().clone();
        let item = original.borrow();
        let detached = Rc::new(RefCell::new(ResourceItem {
            path: value.path,
            resource_type: value.resource_type,
            enabled: false,
            metadata: item.metadata.clone(),
            display_name: item.display_name.clone(),
            group_key: item.group_key.clone(),
            subgroup_key: item.subgroup_key.clone(),
        }));
        drop(item);
        self.component
            .get_resource_list()
            .update_item(detached.clone(), value.enabled);
        self.events.borrow_mut().push(Value::Array(vec![
            json!("detached"),
            json!(detached.borrow().enabled),
        ]));
    }
}

/// A capture-only callback, created outside the nested re-entry body.
fn event_callback(events: Rc<RefCell<Vec<Value>>>, name: &'static str) -> Rc<dyn Fn()> {
    Rc::new(move || events.borrow_mut().push(json!([name])))
}

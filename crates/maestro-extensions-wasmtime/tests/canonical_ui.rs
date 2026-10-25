//! Every canonical UI import forwards to the independently retained native UI.
#![cfg(test)]
use maestro_extensions::{
    EventError, ExtensionUIContext, ExtensionWidgetOptions, WorkingIndicatorOptions,
    types::{ExtensionUIContextPort, NotificationType, WidgetContent},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Original application UI records the operands its methods actually consume.
struct Ui(Rc<RefCell<Vec<Value>>>);
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["notify", message, kind]));
        Ok(())
    }
    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["status", key, text]));
        Ok(())
    }
    fn set_working_message(&self, message: Option<String>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["working", message]));
        Ok(())
    }
    fn set_working_visible(&self, visible: bool) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["visible", visible]));
        Ok(())
    }
    fn set_working_indicator(
        &self,
        options: Option<WorkingIndicatorOptions>,
    ) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!([
            "indicator",
            options.map(|o| json!({"frames":o.frames,"intervalMs":o.interval_ms}))
        ]));
        Ok(())
    }
    fn set_hidden_thinking_label(&self, label: Option<String>) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["hidden", label]));
        Ok(())
    }
    fn set_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let lines = content.map(|c| match c {
            WidgetContent::Lines(lines) => lines,
            WidgetContent::Factory(_) => panic!("expected fixed lines"),
        });
        self.0.borrow_mut().push(json!([
            "widget",
            key,
            lines,
            options.map(|o| json!({"placement":o.placement}))
        ]));
        Ok(())
    }
    fn set_title(&self, title: String) -> Result<(), EventError> {
        self.0.borrow_mut().push(json!(["title", title]));
        Ok(())
    }
}

#[test]
fn canonical_ui_imports_forward_values_and_absence_to_original_ui() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(Rc::new(|_| panic!("no tails")), Rc::new(|_| panic!("no diagnostics")));
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let calls = Rc::new(RefCell::new(Vec::new()));
        runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(Ui(calls.clone())))));
        let handler = runner.get_command("bridge-observe").unwrap().command.handler;
        for (args, expected) in [
            ("ui imports", json!([["notify"," literal notification ","warning"],["status"," literal key ",""],["working"," literal working "],["visible",true],["indicator",{"frames":[],"intervalMs":2.5}],["hidden"," literal hidden "],["widget"," literal widget ",[" literal line ",""],{"placement":"belowEditor"}],["title"," literal title "]])),
            ("ui clear", json!([["notify"," literal notification ",null],["status"," literal key ",null],["working",null],["visible",false],["indicator",null],["hidden",null],["widget"," literal widget ",null,null],["title",""]])),
        ] {
            assert_eq!(handler(args.into(), runner.create_command_context()).await.unwrap_err().to_string(), "UI forwarded");
            assert_eq!(json!(std::mem::take(&mut *calls.borrow_mut())), expected);
        }
        bus.clear();
    });
}

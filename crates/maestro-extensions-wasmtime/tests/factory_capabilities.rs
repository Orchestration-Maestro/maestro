//! Factory metadata reaches the production runtime and its original callback frame.
#![cfg(test)]

use maestro_extensions::{
    ComponentHandle, EventBusController, EventError, ExtensionRunner, ExtensionUIContext,
    ExtensionWidgetOptions, Local, create_event_bus_for,
    types::{ExtensionComponent, ExtensionUIContextPort, UIHostCall, WidgetContent},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[test]
fn canonical_factory_component_preserves_width_and_retained_expansion() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, observe(false));
}

#[test]
fn canonical_factory_expansion_reenters_the_setting_component() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, observe(true));
}

/// The application optionally expands its new component before the setter returns.
struct Ui {
    /// Original terminal factories receive.
    tui: maestro_tui::TUI,
    /// The currently shown factory component.
    shown: RefCell<Option<ExtensionComponent>>,
    /// Whether to invoke expansion from the synchronous setter.
    nested: bool,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn set_widget(
        &self,
        _key: String,
        content: Option<WidgetContent>,
        _: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let Some(WidgetContent::Factory(factory)) = content else {
            self.shown.take();
            return Ok(());
        };
        let built = factory.call((self.tui.clone(), self.theme()))?;
        if self.nested
            && let Some(expansion) = &built.set_expanded
        {
            expansion.call(true)?;
        }
        self.shown.replace(Some(built));
        Ok(())
    }
}

/// A production runner, its host UI and the original bus receiving authored emissions.
struct Fixture {
    /// The original native event bus.
    bus: EventBusController<Local>,
    /// The production component runner.
    runner: ExtensionRunner<Local>,
    /// The application's widget owner.
    ui: Rc<Ui>,
    /// Expansion callback emissions.
    effects: Rc<RefCell<Vec<Value>>>,
}
impl Fixture {
    /// Loads the real authored component through the production artifact factory.
    async fn new(nested: bool) -> Self {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let effects = Rc::new(RefCell::new(Vec::new()));
        let captured = effects.clone();
        bus.on(
            "factory-expanded",
            Rc::new(move |value, _| {
                captured.borrow_mut().push((*value).clone());
                Ok(None)
            }),
        );
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let (tui, _terminal) = super::canonical_ui::screen();
        let ui = Rc::new(Ui {
            tui,
            shown: RefCell::default(),
            nested,
        });
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        Self {
            bus,
            runner,
            ui,
            effects,
        }
    }

    /// Installs the authored factory with independently optional capabilities.
    async fn show(&self, width: Option<f64>, expandable: bool) {
        let handler = self
            .runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let result = handler(
            format!("ui capabilities {}", json!([width, expandable])),
            self.runner.create_command_context(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert_eq!(result, "capabilities shown");
    }

    /// Retains both callable owners after checking the literal width.
    fn retained(&self, width: Option<f64>) -> (ComponentHandle, UIHostCall<bool, ()>) {
        let shown = self.ui.shown.borrow();
        let built = shown.as_ref().unwrap();
        assert_eq!(built.width, width);
        (built.component.clone(), built.set_expanded.clone().unwrap())
    }
}

/// Repeated expansion calls update the same view and preserve the callback's exact error.
fn expansion(component: &ComponentHandle, handler: &UIHostCall<bool, ()>, nested: bool) {
    let initial = if nested {
        "expanded=true width=5"
    } else {
        "expanded=false width=5"
    };
    assert_eq!(component.try_render(5).unwrap(), [initial]);
    handler.call(true).unwrap();
    assert_eq!(
        component.try_render(13).unwrap(),
        ["expanded=true width=13"]
    );
    assert_eq!(
        handler.call(false).unwrap_err().to_string(),
        "expansion rejected 雪\n"
    );
    assert_eq!(component.try_render(9).unwrap(), ["expanded=false width=9"]);
    handler.call(true).unwrap();
}

/// Checks optional absence, retained aliases after replacement and ordinary/nested calls.
async fn observe(nested: bool) {
    let fixture = Fixture::new(nested).await;
    let mut aliases = Vec::new();
    for width in [None, Some(0.0), Some(-3.5), Some(37.25)] {
        fixture.show(width, true).await;
        let (component, handler) = fixture.retained(width);
        expansion(&component, &handler, nested);
        aliases.push((component, handler));
    }
    let expected = if nested {
        vec![json!(true), json!(true), json!(false), json!(true)]
    } else {
        vec![json!(true), json!(false), json!(true)]
    };
    let expected: Vec<_> = (0..4).flat_map(|_| expected.clone()).collect();
    assert_eq!(*fixture.effects.borrow(), expected);
    fixture.show(None, false).await;
    let built = fixture.ui.shown.take().unwrap();
    assert_eq!(built.width, None);
    assert!(built.set_expanded.is_none());
    assert_eq!(
        built.component.try_render(7).unwrap(),
        ["expanded=false width=7"]
    );
    for (component, handler) in aliases {
        assert_eq!(
            handler.call(false).unwrap_err().to_string(),
            "expansion rejected 雪\n"
        );
        assert_eq!(component.try_render(3).unwrap(), ["expanded=false width=3"]);
    }
    fixture.bus.clear();
}

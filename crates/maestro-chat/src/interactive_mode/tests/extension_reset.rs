//! What session replacement resets of the extension presentation, and what persists
//! across prompt turns until it does.
use super::extension_custom::{Build, Flow, Views, overlay, plain};
use super::extension_editors::{Knobs, probe_factory};
use super::extension_ui::port;
use super::extension_widgets::with_factories;
use super::{World, isolated, with_held_response};
use maestro_app::{
    ComponentFactory, EditorFactory, EventError, ExtensionAPI, ExtensionComponent,
    ExtensionUIContext, FooterFactory, Local, NewSessionCommandOptions, ReadonlyFooterDataProvider,
    TerminalInputHandler, WidgetContent, WorkingIndicatorOptions,
};
use maestro_tui::tui::{ComponentHandle, InputListenerResult};
use maestro_tui::{Component, EditorComponent, Text};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// An error whose allocation identity survives host forwarding.
#[derive(Debug)]
struct Original(Arc<()>);
impl std::fmt::Display for Original {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("reset cleanup failed")
    }
}
impl std::error::Error for Original {}

/// What a witness's widget disposals record, do and fail with.
#[derive(Default)]
struct Disposals {
    /// Effects in call order.
    trace: RefCell<Vec<String>>,
    /// Run by the disposal of `a`.
    edit: RefCell<Option<Box<dyn Fn()>>>,
    /// The widget whose disposal fails.
    fails: Cell<Option<&'static str>>,
    /// The identity the failure carries.
    error: Arc<()>,
}

/// A widget whose disposal records `label`, runs the witness's edit for `a` and fails
/// when the witness names it.
fn widget(
    api: &ExtensionAPI<Local>,
    label: &'static str,
    disposals: &Rc<Disposals>,
) -> WidgetContent {
    let (factory_api, disposals) = (api.clone(), disposals.clone());
    WidgetContent::Factory(api.ui_callback(move |_, _| {
        let disposals = disposals.clone();
        Ok(ExtensionComponent {
            component: Rc::new(Text::new(label.into(), 0, 0, None)),
            dispose: Some(factory_api.ui_callback(move |(), _| disposals.dispose(label))),
            set_expanded: None,
            width: None,
        })
    }))
}

impl Disposals {
    /// Records the disposal of `label`, then fails or runs the edit as the witness selects.
    fn dispose(&self, label: &'static str) -> Result<(), EventError> {
        self.trace.borrow_mut().push(format!("{label}:dispose"));
        if self.fails.get() == Some(label) {
            return Err(Box::new(Original(self.error.clone())));
        }
        if label == "a"
            && let Some(edit) = self.edit.borrow().as_ref()
        {
            edit();
        }
        Ok(())
    }
}

/// The keys each placement holds, in order.
fn widget_keys(world: &World) -> (Vec<String>, Vec<String>) {
    let keys = |map: &maestro_app::Live<String, Rc<ExtensionComponent>>| {
        map.iter().map(|(key, _)| key).collect()
    };
    (
        keys(&world.mode.extension_widgets_above),
        keys(&world.mode.extension_widgets_below),
    )
}

/// The edit the first disposal makes to the live widgets, by case.
fn live_edit(ui: &ExtensionUIContext, world: &World, case: &str) -> Box<dyn Fn()> {
    let (ui, mode) = (ui.clone(), Rc::downgrade(&world.mode));
    let line = |text: &str| Some(WidgetContent::Lines(vec![text.into()]));
    match case {
        "append" => Box::new(move || ui.set_widget("d".into(), line("d"), None).unwrap()),
        "remove-next" => Box::new(move || ui.set_widget("b".into(), None, None).unwrap()),
        "replace-next" => Box::new(move || ui.set_widget("b".into(), line("b2"), None).unwrap()),
        _ => Box::new(move || {
            mode.upgrade().unwrap().extension_widgets_above.clear();
            ui.set_widget("e".into(), line("e"), None).unwrap();
        }),
    }
}

#[test]
fn widget_clear_observes_finite_live_disposer_edits() {
    isolated(
        "widget_clear_observes_finite_live_disposer_edits",
        async |directory| {
            for (case, expected) in [
                ("append", vec!["a:dispose", "b:dispose", "c:dispose"]),
                ("remove-next", vec!["a:dispose", "b:dispose", "c:dispose"]),
                ("replace-next", vec!["a:dispose", "b:dispose", "c:dispose"]),
                ("clear-then-append", vec!["a:dispose"]),
                ("fail", vec!["a:dispose", "b:dispose", "c:dispose"]),
            ] {
                let disposals = Rc::new(Disposals::default());
                let witness = disposals.clone();
                let (world, widgets) = with_factories(directory, move |api| {
                    ["a", "b", "c"]
                        .map(|label| widget(api, label, &witness))
                        .to_vec()
                })
                .await;
                let ui = port(&world.mode);
                for (label, content) in ["a", "b", "c"].into_iter().zip(widgets) {
                    ui.set_widget(label.into(), Some(content), None).unwrap();
                }
                if case == "fail" {
                    disposals.fails.set(Some("c"));
                } else {
                    disposals.edit.replace(Some(live_edit(&ui, &world, case)));
                }
                let result = world.mode.clear_extension_widgets();
                assert_eq!(*disposals.trace.borrow(), expected, "{case}");
                if case == "fail" {
                    let error = result.unwrap_err();
                    assert!(error.downcast_ref::<Original>().is_some());
                    assert_eq!(widget_keys(&world).0, ["a", "b", "c"]);
                } else {
                    result.unwrap();
                    assert_eq!(widget_keys(&world), (Vec::new(), Vec::new()));
                }
                disposals.edit.replace(None);
            }
        },
    );
}

/// The callbacks the replacement witness installs.
struct Kit {
    /// A traced header.
    header: ComponentFactory,
    /// A traced footer.
    footer: FooterFactory,
    /// A traced widget.
    widget: WidgetContent,
    /// An application-capable editor.
    editor: EditorFactory,
    /// A traced terminal input listener.
    input: TerminalInputHandler,
    /// One ready overlay view and one view built later in place of the editor.
    views: super::extension_custom::Bound,
}

/// A traced component disposing as `label`.
fn traced(
    api: &ExtensionAPI<Local>,
    label: &'static str,
    disposals: &Rc<Disposals>,
) -> ExtensionComponent {
    let disposals = disposals.clone();
    ExtensionComponent {
        component: Rc::new(Text::new(label.into(), 0, 0, None)),
        dispose: Some(api.ui_callback(move |(), _| disposals.dispose(label))),
        set_expanded: None,
        width: None,
    }
}

/// One ready overlay view and one later view in place of the editor.
const VIEWS: &[(&str, Build)] = &[
    ("CUSTOM", plain(Flow::Ready)),
    ("CUSTOM", plain(Flow::Async)),
];

/// Binds the witness's callbacks.
fn kit(
    api: &ExtensionAPI<Local>,
    disposals: &Rc<Disposals>,
    views: &Rc<Views>,
    knobs: &Rc<Knobs>,
) -> Kit {
    let (header_api, footer_api) = (api.clone(), api.clone());
    let (header_disposals, footer_disposals, input) =
        (disposals.clone(), disposals.clone(), disposals.clone());
    Kit {
        header: api.ui_callback(move |_, _| Ok(traced(&header_api, "H", &header_disposals))),
        footer: api.ui_callback(move |_, _| Ok(traced(&footer_api, "F", &footer_disposals))),
        widget: widget(api, "W", disposals),
        editor: probe_factory(api, knobs, true, None),
        input: api.ui_callback(move |_: String, _| {
            input.trace.borrow_mut().push("input".into());
            Ok(InputListenerResult::Consume)
        }),
        views: views.bind(api, VIEWS),
    }
}

/// Installs every native addition, with three dialogs and two custom views pending.
fn install_everything(
    world: &World,
    kit: &Kit,
    knobs: &Knobs,
) -> Vec<maestro_app::ExtensionUIFuture<Option<String>>> {
    let ui = ExtensionUIContext::new(world.mode.clone());
    ui.set_editor_component(Some(kit.editor.clone())).unwrap();
    ui.set_header(Some(kit.header.clone())).unwrap();
    ui.set_footer(Some(kit.footer.clone())).unwrap();
    ui.set_widget("W".into(), Some(kit.widget.clone()), None)
        .unwrap();
    ui.set_status("s".into(), Some("x".into())).unwrap();
    drop(ui.on_terminal_input(kit.input.clone()).unwrap());
    let pending = vec![
        ui.select("pick".into(), vec!["a".into()], None),
        ui.input("in".into(), None, None),
        ui.editor("ed".into(), None),
        ui.custom(kit.views.views[0].clone(), Some(overlay(None, None))),
        ui.custom(kit.views.views[1].clone(), None),
    ];
    knobs.editor().inner.set_text("current");
    pending
}

/// Checks that the replacement reset every native addition and kept the current text.
fn assert_reset(world: &World, disposals: &Disposals) {
    assert_eq!(
        *disposals.trace.borrow(),
        ["CUSTOM:dispose", "F:dispose", "H:dispose", "W:dispose"]
    );
    assert!(!world.mode.ui.has_overlay());
    let built_in: ComponentHandle = world.mode.footer.clone();
    let last = world.mode.ui.children().borrow().last().cloned().unwrap();
    assert!(std::ptr::addr_eq(Rc::as_ptr(&last), Rc::as_ptr(&built_in)));
    assert!(world.mode.custom_header.borrow().is_none());
    assert!(world.mode.extension_widgets_above.is_empty());
    let statuses = world.mode.footer_data_provider.get_extension_statuses();
    assert!(statuses.iter().next().is_none());
    assert!(
        ExtensionUIContext::new(world.mode.clone())
            .get_editor_component()
            .unwrap()
            .is_none()
    );
    assert!(!world.mode.custom_editor_current());
    assert_eq!(world.mode.editor.get_text(), "current");
    assert!(world.mode.editor.on_extension_shortcut().is_none());
    world.terminal.send_input("x");
    assert!(!disposals.trace.borrow().contains(&"input".to_owned()));
}

#[test]
fn session_replacement_resets_every_native_addition() {
    isolated(
        "session_replacement_resets_every_native_addition",
        async |directory| {
            let (disposals, views, knobs) = (
                Rc::new(Disposals::default()),
                Views::new(),
                Rc::new(Knobs::default()),
            );
            let (d, v, k) = (disposals.clone(), views.clone(), knobs.clone());
            let (world, mut kits) =
                with_factories(directory, move |api| vec![kit(api, &d, &v, &k)]).await;
            let kit = kits.pop().unwrap();
            for _ in 0..2 {
                disposals.trace.borrow_mut().clear();
                views.trace.borrow_mut().clear();
                let pending = install_everything(&world, &kit, &knobs);
                world
                    .host
                    .new_session(NewSessionCommandOptions::default())
                    .await
                    .unwrap();
                for result in pending {
                    assert_eq!(result.await.unwrap(), None);
                }
                views
                    .trace
                    .borrow_mut()
                    .iter()
                    .for_each(|e| disposals.trace.borrow_mut().insert(0, e.clone()));
                assert_reset(&world, &disposals);
                views.release(&world).await;
                assert_eq!(*views.trace.borrow(), ["CUSTOM:dispose", "CUSTOM:dispose"]);
                assert!(!world.mode.ui.has_overlay());
            }
            let old = world.host.session();
            install_everything(&world, &kit, &knobs);
            disposals.fails.set(Some("W"));
            let error = world
                .host
                .new_session(NewSessionCommandOptions::default())
                .await
                .unwrap_err();
            assert!(error.to_string().contains("reset cleanup failed"));
            assert!(Rc::ptr_eq(&old, &world.host.session()));
            assert!(world.mode.custom_header.borrow().is_none());
            assert!(!world.mode.extension_widgets_above.is_empty());
            let reset = world.mode.reset_extension_ui().unwrap_err();
            assert!(
                reset
                    .downcast_ref::<Original>()
                    .is_some_and(|e| Arc::ptr_eq(&e.0, &disposals.error))
            );
        },
    );
}

/// The working row's text while a response runs.
fn working_row(world: &World) -> String {
    let rows = world.mode.status_container.render(80);
    maestro_app::strip_ansi(&rows.join("\n")).unwrap()
}

#[test]
fn loader_customizations_survive_multiple_prompt_turns() {
    isolated(
        "loader_customizations_survive_multiple_prompt_turns",
        async |directory| {
            let world = super::started(directory).await;
            let ui = port(&world.mode);
            ui.set_working_message(Some("Busy".into())).unwrap();
            let frames = WorkingIndicatorOptions {
                frames: Some(vec!["@".into()]),
                interval_ms: None,
            };
            ui.set_working_indicator(Some(frames)).unwrap();
            ui.set_hidden_thinking_label(Some("Hidden".into())).unwrap();
            for _ in 0..2 {
                let row = with_held_response(&world, async |_| working_row(&world)).await;
                assert!(row.contains("@ Busy"), "{row}");
                assert_eq!(*world.mode.working.hidden_thinking_label.borrow(), "Hidden");
            }
            ui.set_working_visible(false).unwrap();
            for _ in 0..2 {
                let row = with_held_response(&world, async |_| working_row(&world)).await;
                assert!(!row.contains("Busy"), "{row}");
            }
            world
                .host
                .new_session(NewSessionCommandOptions::default())
                .await
                .unwrap();
            let row = with_held_response(&world, async |_| working_row(&world)).await;
            assert!(row.contains("Working..."), "{row}");
            assert!(!row.contains('@'), "{row}");
            let default = crate::AssistantMessageOptions::default().hidden_thinking_label;
            assert_eq!(*world.mode.working.hidden_thinking_label.borrow(), default);
        },
    );
}

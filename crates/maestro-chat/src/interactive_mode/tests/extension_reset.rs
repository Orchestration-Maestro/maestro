//! What session replacement resets of the extension presentation, and what persists
//! across prompt turns until it does.
use super::extension_custom::{Build, Flow, Views, overlay, plain};
use super::extension_editors::{Knobs, probe_factory};
use super::extension_ui::port;
use super::extension_widgets::with_factories;
use super::{World, isolated, with_held_response};
use maestro_app::{
    Cancellation, ComponentFactory, EditorFactory, EventError, ExtensionAPI, ExtensionComponent,
    ExtensionUIContext, ExtensionUIDialogOptions, ExtensionWidgetOptions, FooterFactory, Local,
    NewSessionCommandOptions, ReadonlyFooterDataProvider, TerminalInputHandler, WidgetContent,
    WidgetPlacement, WorkingIndicatorOptions,
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
fn live_edit(
    ui: &ExtensionUIContext,
    world: &World,
    case: &str,
    added: WidgetContent,
) -> Box<dyn Fn()> {
    let (ui, mode) = (ui.clone(), Rc::downgrade(&world.mode));
    match case {
        "append" => Box::new(move || {
            ui.set_widget("d".into(), Some(added.clone()), None)
                .unwrap();
        }),
        "remove-next" => Box::new(move || ui.set_widget("b".into(), None, None).unwrap()),
        "replace-next" => Box::new(move || {
            ui.set_widget("b".into(), Some(added.clone()), None)
                .unwrap();
        }),
        _ => Box::new(move || {
            mode.upgrade().unwrap().extension_widgets_above.clear();
            ui.set_widget("d".into(), Some(added.clone()), None)
                .unwrap();
        }),
    }
}

/// Live edits and their independently expected disposal order.
const CLEAR_CASES: &[(&str, &[&str])] = &[
    (
        "append",
        &[
            "a:dispose",
            "b:dispose",
            "c:dispose",
            "d:dispose",
            "e:dispose",
        ],
    ),
    (
        "remove-next",
        &["a:dispose", "b:dispose", "c:dispose", "e:dispose"],
    ),
    (
        "replace-next",
        &[
            "a:dispose",
            "b:dispose",
            "c:dispose",
            "d:dispose",
            "e:dispose",
        ],
    ),
    (
        "clear-then-append",
        &["a:dispose", "d:dispose", "e:dispose"],
    ),
    ("fail", &["a:dispose", "b:dispose", "c:dispose"]),
];

#[test]
fn widget_clear_observes_finite_live_disposer_edits() {
    isolated(
        "widget_clear_observes_finite_live_disposer_edits",
        async |directory| {
            for &(case, expected) in CLEAR_CASES {
                let disposals = Rc::new(Disposals::default());
                let witness = disposals.clone();
                let (world, mut widgets) = with_factories(directory, move |api| {
                    ["a", "b", "c", "d", "e"]
                        .map(|label| widget(api, label, &witness))
                        .to_vec()
                })
                .await;
                let ui = port(&world.mode);
                let below = widgets.pop().unwrap();
                let added = widgets.pop().unwrap();
                ui.set_widget(
                    "below".into(),
                    Some(below),
                    Some(ExtensionWidgetOptions {
                        placement: Some(WidgetPlacement::BelowEditor),
                    }),
                )
                .unwrap();
                for (label, content) in ["a", "b", "c"].into_iter().zip(widgets) {
                    ui.set_widget(label.into(), Some(content), None).unwrap();
                }
                if case == "fail" {
                    disposals.fails.set(Some("c"));
                } else {
                    disposals
                        .edit
                        .replace(Some(live_edit(&ui, &world, case, added)));
                }
                let result = world.mode.clear_extension_widgets();
                assert_eq!(disposals.trace.borrow().as_slice(), expected, "{case}");
                if case == "fail" {
                    let error = result.unwrap_err();
                    assert!(
                        error
                            .downcast_ref::<Original>()
                            .is_some_and(|error| Arc::ptr_eq(&error.0, &disposals.error))
                    );
                    assert_eq!(
                        widget_keys(&world),
                        (
                            vec!["a".into(), "b".into(), "c".into()],
                            vec!["below".into()]
                        )
                    );
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

/// A shown inline view, two shown overlays and an inline and overlay build still pending.
const VIEWS: &[(&str, Build)] = &[
    ("INLINE", plain(Flow::Ready)),
    ("A", plain(Flow::Ready)),
    ("B", plain(Flow::Ready)),
    ("LATE-I", plain(Flow::Async)),
    ("LATE-O", plain(Flow::Async)),
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

/// Installs every native addition, with three dialogs and five custom operations pending.
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
    let options = || {
        Some(ExtensionUIDialogOptions {
            signal: Some(Cancellation::new()),
            timeout: Some(5000.0),
        })
    };
    let pending = vec![
        ui.select("pick".into(), vec!["a".into()], options()),
        ui.input("in".into(), None, options()),
        ui.editor("ed".into(), None),
        ui.custom(kit.views.views[0].clone(), None),
        ui.custom(kit.views.views[1].clone(), Some(overlay(None, None))),
        ui.custom(kit.views.views[2].clone(), Some(overlay(None, None))),
        ui.custom(kit.views.views[3].clone(), None),
        ui.custom(kit.views.views[4].clone(), Some(overlay(None, None))),
    ];
    knobs.editor().inner.set_text("current");
    pending
}

/// Checks that the replacement reset every native addition and kept the current text.
fn assert_reset(world: &World, disposals: &Disposals) {
    assert_eq!(
        *disposals.trace.borrow(),
        ["F:dispose", "H:dispose", "W:dispose"]
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

/// Verifies every custom observation settled, hidden dialog work ended and late builds dispose.
fn assert_abandoned(
    world: &World,
    views: &Views,
    pending: Vec<maestro_app::ExtensionUIFuture<Option<String>>>,
) {
    for mut result in pending {
        assert_eq!(
            super::typed::poll(&mut result)
                .expect("replacement settles every admitted observation")
                .unwrap(),
            None
        );
    }
    assert_eq!(
        *views.trace.borrow(),
        ["INLINE:dispose", "A:dispose", "B:dispose"]
    );
    assert!(world.runtime.drain_futures().is_empty());
    let later = std::time::Duration::from_millis(world.runtime.millis() + 1000);
    for result in world.runtime.advance_to(later) {
        result.unwrap();
    }
    assert_eq!(world.runtime.pending(), 0, "both hidden countdowns stopped");
    assert_eq!(
        world.runtime.futures(),
        2,
        "only the admitted builds remain"
    );
    views.release(world);
    views.release(world);
    assert_eq!(
        *views.trace.borrow(),
        [
            "INLINE:dispose",
            "A:dispose",
            "B:dispose",
            "LATE-I:dispose",
            "LATE-O:dispose"
        ]
    );
    assert!(!world.mode.ui.has_overlay());
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
                assert_reset(&world, &disposals);
                assert_abandoned(&world, &views, pending);
            }
            let old = world.host.session();
            views.trace.borrow_mut().clear();
            let pending = install_everything(&world, &kit, &knobs);
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
            assert_abandoned(&world, &views, pending);
            let reset = world.mode.reset_extension_ui().unwrap_err();
            assert!(
                reset
                    .downcast_ref::<Original>()
                    .is_some_and(|e| Arc::ptr_eq(&e.0, &disposals.error))
            );
        },
    );
}

#[test]
fn hidden_dialog_completion_preserves_the_active_panel() {
    isolated(
        "hidden_dialog_completion_preserves_the_active_panel",
        async |directory| {
            let world = super::started(directory).await;
            let ui = port(&world.mode);
            let mut selected = ui.select("pick".into(), vec!["answer".into()], None);
            let selector = world.mode.editor_container.children().borrow()[0].clone();
            let mut input = ui.input("input".into(), None, None);
            let active = world.mode.editor_container.children().borrow()[0].clone();
            selector.input_handler().unwrap().handle_input("\x1b");
            assert_eq!(super::typed::poll(&mut selected).unwrap().unwrap(), None);
            let shown = world.mode.editor_container.children().borrow()[0].clone();
            assert!(
                Rc::ptr_eq(&shown, &active),
                "a hidden dialog cannot replace the active input"
            );
            let mut next = ui.input("next".into(), None, None);
            assert!(
                super::typed::poll(&mut input).is_none(),
                "the covered input stays open"
            );
            active.input_handler().unwrap().handle_input("\r");
            assert_eq!(
                super::typed::poll(&mut input).unwrap().unwrap(),
                Some(String::new())
            );
            assert!(super::typed::poll(&mut next).is_none());
            let shown = world.mode.editor_container.children().borrow()[0].clone();
            assert!(!Rc::ptr_eq(&shown, &active), "the shown dialog stays");
            world.terminal.send_input("\x1b");
            assert_eq!(next.await.unwrap(), None);
            let shown = world.mode.editor_container.children().borrow()[0].clone();
            let editor: ComponentHandle = world.mode.editor.clone();
            assert!(Rc::ptr_eq(&shown, &editor));
            let weak = Rc::downgrade(&world.mode);
            drop((ui, world));
            assert!(weak.upgrade().is_none());
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

#[test]
fn reset_shows_the_default_message_on_a_running_loader() {
    isolated(
        "reset_shows_the_default_message_on_a_running_loader",
        async |directory| {
            let world = super::started(directory).await;
            port(&world.mode)
                .set_working_message(Some("Busy".into()))
                .unwrap();
            let (before, after) = with_held_response(&world, async |_| {
                let before = working_row(&world);
                world.mode.reset_extension_ui().unwrap();
                (before, working_row(&world))
            })
            .await;
            assert!(before.contains("Busy"), "{before}");
            let interrupt = maestro_app::key_text("app.interrupt");
            let message = format!("Working... ({interrupt} to interrupt)");
            assert!(after.contains(&message), "{after}");
            assert!(!after.contains("Busy"), "{after}");
        },
    );
}

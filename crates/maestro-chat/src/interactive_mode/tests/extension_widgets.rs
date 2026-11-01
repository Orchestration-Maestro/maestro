//! Native widget and status calls through the retained UI host.
use super::extension_ui::port;
use super::{Setup, World, isolated, started};
use maestro_app::{
    ExtensionAPI, ExtensionComponent, ExtensionFactory, ExtensionWidgetOptions, Local,
    ReadonlyFooterDataProvider, SettingsManager, WidgetContent, WidgetPlacement,
};
use maestro_tui::{Component, Text};
use serde::Deserialize;
use std::{cell::RefCell, path::Path, rc::Rc};

/// Source-recorded text input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lines {
    lines: Vec<String>,
}
/// Source-recorded placement and line count.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Placement {
    placement: Option<WidgetPlacement>,
    count: usize,
}
/// Complete painted rows on each side of the editor.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paint {
    above: Vec<String>,
    below: Vec<String>,
}
/// Paint and retained membership for a placement case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Placed {
    above: Vec<String>,
    below: Vec<String>,
    above_keys: Vec<String>,
    below_keys: Vec<String>,
}
/// Retained key order on each side.
fn keys(world: &World) -> (Vec<String>, Vec<String>) {
    (
        world
            .mode
            .extension_widgets_above
            .iter()
            .map(|(key, _)| key)
            .collect(),
        world
            .mode
            .extension_widgets_below
            .iter()
            .map(|(key, _)| key)
            .collect(),
    )
}
/// Both actual widget containers at the recording terminal width.
fn paint(world: &World) -> (Vec<String>, Vec<String>) {
    (
        world.mode.widget_container_above.render(80),
        world.mode.widget_container_below.render(80),
    )
}
/// Loads real bound callbacks with the existing session and terminal fixture.
pub(super) async fn with_factories<T: 'static>(
    directory: &Path,
    build: impl Fn(&ExtensionAPI<Local>) -> Vec<T> + 'static,
) -> (World, Vec<T>) {
    with_configured_factories(directory, Box::new(|_| {}), build).await
}

/// Loads real bound callbacks as [`with_factories`] does, after `configure` changes the
/// settings.
pub(super) async fn with_configured_factories<T: 'static>(
    directory: &Path,
    configure: Box<dyn FnOnce(&mut SettingsManager)>,
    build: impl Fn(&ExtensionAPI<Local>) -> Vec<T> + 'static,
) -> (World, Vec<T>) {
    let stored = Rc::new(RefCell::new(Vec::new()));
    let output = stored.clone();
    let factory: ExtensionFactory<Local> = Rc::new(move |api| {
        output.replace(build(&api));
        Box::pin(std::future::ready(Ok(())))
    });
    let world = Setup {
        extension: Some(factory),
        configure,
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    let callbacks = stored.take();
    (world, callbacks)
}

#[test]
fn widgets_bound_text_rows_and_placement() {
    isolated("widgets_bound_text_rows_and_placement", async |directory| {
        let world = started(directory).await;
        let ui = port(&world.mode);
        for (input, expected) in super::cases::<Placement, Placed>(
            include_str!("../../../tests/fixtures/interactive_extension_widgets.json"),
            "widgets_bound_text_rows_and_placement",
        ) {
            ui.set_widget(
                "b".into(),
                Some(WidgetContent::Lines(
                    (0..input.count).map(|n| format!("L{n}")).collect(),
                )),
                Some(ExtensionWidgetOptions {
                    placement: input.placement,
                }),
            )
            .unwrap();
            assert_eq!(
                paint(&world),
                (
                    super::painted(&world.themes.theme().get().unwrap(), &expected.above),
                    super::painted(&world.themes.theme().get().unwrap(), &expected.below)
                )
            );
            assert_eq!(keys(&world), (expected.above_keys, expected.below_keys));
        }
        ui.set_widget(
            "second".into(),
            Some(WidgetContent::Lines(
                (0..11).map(|_| "second".into()).collect(),
            )),
            None,
        )
        .unwrap();
        assert_eq!(world.mode.widget_container_above.render(80).len(), 12);
        ui.set_widget(
            "third".into(),
            Some(WidgetContent::Lines(
                (0..11).map(|_| "third".into()).collect(),
            )),
            None,
        )
        .unwrap();
        assert_eq!(world.mode.widget_container_above.render(80).len(), 23);
    });
}

#[test]
fn status_updates_retain_live_order_and_request_render() {
    isolated(
        "status_updates_retain_live_order_and_request_render",
        async |directory| {
            let world = started(directory).await;
            let ui = port(&world.mode);
            let live = world.mode.footer_data_provider.get_extension_statuses();
            for (key, text, next) in [
                ("z", Some("one"), vec![("z", "one")]),
                ("a", Some("two"), vec![("z", "one"), ("a", "two")]),
                ("z", Some("three"), vec![("z", "three"), ("a", "two")]),
                ("", Some(""), vec![("z", "three"), ("a", "two"), ("", "")]),
                ("a", None, vec![("z", "three"), ("", "")]),
                (
                    "a",
                    Some("last"),
                    vec![("z", "three"), ("", ""), ("a", "last")],
                ),
            ] {
                world.runtime.settle().unwrap();
                assert_eq!(world.runtime.pending(), 0);
                ui.set_status(key.into(), text.map(str::to_owned)).unwrap();
                assert!(world.runtime.pending() > 0);
                let expected: Vec<(String, String)> = next
                    .into_iter()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect();
                assert_eq!(live.iter().collect::<Vec<_>>(), expected);
            }
        },
    );
}

#[test]
fn widgets_keep_empty_unicode_multiline_and_visible_styles() {
    isolated(
        "widgets_keep_empty_unicode_multiline_and_visible_styles",
        async |directory| {
            let world = started(directory).await;
            let ui = port(&world.mode);
            for (input, expected) in super::cases::<Lines, Paint>(
                include_str!("../../../tests/fixtures/interactive_extension_widgets.json"),
                "widgets_keep_empty_unicode_multiline_and_visible_styles",
            ) {
                ui.set_widget("text".into(), Some(WidgetContent::Lines(input.lines)), None)
                    .unwrap();
                assert_eq!(
                    paint(&world),
                    (
                        super::painted(&world.themes.theme().get().unwrap(), &expected.above),
                        super::painted(&world.themes.theme().get().unwrap(), &expected.below)
                    )
                );
            }
        },
    );
}

/// A component factory whose two phases record their exact order.
fn traced(
    api: &ExtensionAPI<Local>,
    label: &'static str,
    trace: &Rc<RefCell<Vec<String>>>,
) -> WidgetContent {
    let factory_trace = trace.clone();
    let dispose_trace = trace.clone();
    let dispose = api.ui_callback(move |(), _| {
        dispose_trace.borrow_mut().push(format!("{label}:dispose"));
        Ok(())
    });
    WidgetContent::Factory(api.ui_callback(move |_, _| {
        factory_trace.borrow_mut().push(format!("{label}:factory"));
        Ok(ExtensionComponent {
            component: Rc::new(Text::new(label.into(), 0, 0, None)),
            dispose: Some(dispose.clone()),
            set_expanded: None,
            width: None,
        })
    }))
}

#[test]
fn widgets_replacement_reorders_moves_and_disposes() {
    isolated(
        "widgets_replacement_reorders_moves_and_disposes",
        async |directory| {
            let trace = Rc::new(RefCell::new(Vec::new()));
            let recorded = trace.clone();
            let (world, callbacks) = with_factories(directory, move |api| {
                ["B", "A", "B2", "A2"]
                    .into_iter()
                    .map(|label| traced(api, label, &recorded))
                    .collect()
            })
            .await;
            let ui = port(&world.mode);
            let mut callbacks = callbacks.into_iter();
            ui.set_widget("b".into(), callbacks.next(), None).unwrap();
            ui.set_widget("a".into(), callbacks.next(), None).unwrap();
            ui.set_widget("b".into(), callbacks.next(), None).unwrap();
            assert_eq!(keys(&world), (vec!["a".into(), "b".into()], vec![]));
            assert_eq!(
                paint(&world).0,
                [
                    String::new(),
                    format!("{:<80}", "A"),
                    format!("{:<80}", "B2")
                ]
            );
            ui.set_widget(
                "a".into(),
                callbacks.next(),
                Some(ExtensionWidgetOptions {
                    placement: Some(WidgetPlacement::BelowEditor),
                }),
            )
            .unwrap();
            assert_eq!(keys(&world), (vec!["b".into()], vec!["a".into()]));
            for key in ["b", "a", "a"] {
                ui.set_widget(key.into(), None, None).unwrap();
            }
            assert_eq!(paint(&world), (vec![String::new()], vec![]));
            assert_eq!(
                *trace.borrow(),
                [
                    "B:factory",
                    "A:factory",
                    "B:dispose",
                    "B2:factory",
                    "A:dispose",
                    "A2:factory",
                    "B2:dispose",
                    "A2:dispose"
                ]
            );
        },
    );
}

/// An error whose allocation identity survives host forwarding.
#[derive(Debug)]
struct Original(std::sync::Arc<()>);
impl std::fmt::Display for Original {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("widget callback failed")
    }
}
impl std::error::Error for Original {}

/// Builds the two original-error branches with their ordered disposal/factory trace.
fn failing_factories(
    api: &ExtensionAPI<Local>,
    dispose_fails: bool,
    returned: &std::sync::Arc<()>,
    recorded: &Rc<RefCell<Vec<&'static str>>>,
) -> Vec<WidgetContent> {
    let disposed = recorded.clone();
    let failure = returned.clone();
    let dispose = api.ui_callback(move |(), _| {
        disposed.borrow_mut().push("dispose-old");
        if dispose_fails {
            Err(Box::new(Original(failure.clone())))
        } else {
            Ok(())
        }
    });
    let old = api.ui_callback(move |_, _| {
        Ok(ExtensionComponent {
            component: Rc::new(Text::new("OLD".into(), 0, 0, None)),
            dispose: Some(dispose.clone()),
            set_expanded: None,
            width: None,
        })
    });
    let called = recorded.clone();
    let failure = returned.clone();
    let new = api.ui_callback(move |_, _| {
        called.borrow_mut().push("new-factory");
        Err(Box::new(Original(failure.clone())))
    });
    vec![WidgetContent::Factory(old), WidgetContent::Factory(new)]
}

#[test]
fn widget_failures_preserve_original_error_and_prior_effects() {
    isolated(
        "widget_failures_preserve_original_error_and_prior_effects",
        async |directory| {
            for dispose_fails in [false, true] {
                let identity = std::sync::Arc::new(());
                let returned = identity.clone();
                let trace = Rc::new(RefCell::new(Vec::new()));
                let recorded = trace.clone();
                let (world, callbacks) = with_factories(directory, move |api| {
                    failing_factories(api, dispose_fails, &returned, &recorded)
                })
                .await;
                let ui = port(&world.mode);
                let mut callbacks = callbacks.into_iter();
                ui.set_widget("a".into(), callbacks.next(), None).unwrap();
                world.runtime.settle().unwrap();
                let error = ui
                    .set_widget("a".into(), callbacks.next(), None)
                    .unwrap_err();
                assert!(std::sync::Arc::ptr_eq(
                    &error.downcast_ref::<Original>().unwrap().0,
                    &identity
                ));
                assert_eq!(
                    world.mode.extension_widgets_above.contains_key("a"),
                    dispose_fails
                );
                assert_eq!(paint(&world).0, [String::new(), format!("{:<80}", "OLD")]);
                assert_eq!(world.runtime.pending(), 0);
                assert_eq!(
                    *trace.borrow(),
                    if dispose_fails {
                        vec!["dispose-old"]
                    } else {
                        vec!["dispose-old", "new-factory"]
                    }
                );
                if !dispose_fails {
                    ui.set_widget("a".into(), None, None).unwrap();
                    assert_eq!(paint(&world).0, [""]);
                }
            }
        },
    );
}

/// Toolkit operands retained by the extension callback.
type WidgetOperands = Rc<RefCell<Option<(maestro_tui::TUI, maestro_theme::LiveTheme)>>>;

/// Subscribes one origin to the witness channel.
fn observe(
    bus: &maestro_app::EventBus<Local>,
    trace: Rc<RefCell<Vec<String>>>,
    label: &'static str,
) {
    bus.on(
        "widget",
        Rc::new(move |_, _| {
            trace.borrow_mut().push(label.into());
            Ok(None)
        }),
    );
}

/// Binds the component and disposer to one origin without retaining the host.
fn origin_factory(
    recorded: Rc<RefCell<Vec<String>>>,
    output: Rc<RefCell<Option<WidgetContent>>>,
    released: Rc<()>,
    observed: WidgetOperands,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        observe(&api.events, recorded.clone(), "same");
        let factory_trace = recorded.clone();
        let capture = released.clone();
        let observed = observed.clone();
        let disposed = recorded.clone();
        let held = released.clone();
        let dispose = api.ui_callback(move |(), emitter| {
            let _ = &held;
            disposed.borrow_mut().push("dispose-before".into());
            emitter.emit("widget", serde_json::Value::Null);
            disposed.borrow_mut().push("dispose-after".into());
            Ok(())
        });
        output.replace(Some(WidgetContent::Factory(api.ui_callback(
            move |(ui, theme), emitter| {
                observed.replace(Some((ui, theme)));
                let _ = &capture;
                factory_trace.borrow_mut().push("before".into());
                emitter.emit("widget", serde_json::Value::Null);
                assert_eq!(*factory_trace.borrow(), ["before", "same"]);
                factory_trace.borrow_mut().push("after".into());
                Ok(ExtensionComponent {
                    component: Rc::new(Text::new("bound".into(), 0, 0, None)),
                    dispose: Some(dispose.clone()),
                    set_expanded: None,
                    width: None,
                })
            },
        ))));
        Box::pin(std::future::ready(Ok(())))
    })
}

#[test]
fn widget_factory_callbacks_retain_origin_and_release_the_last_owner() {
    isolated(
        "widget_factory_callbacks_retain_origin_and_release_the_last_owner",
        async |directory| {
            let trace = Rc::new(RefCell::new(Vec::<String>::new()));
            let controller =
                maestro_app::create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
            let foreign = controller.for_extension();
            observe(&foreign, trace.clone(), "foreign");
            let stored = Rc::new(RefCell::new(None));
            let released = Rc::new(());
            let weak_capture = Rc::downgrade(&released);
            let operands = Rc::new(RefCell::new(None));
            let loaded = super::loaded_on_bus(
                directory.to_str().unwrap(),
                origin_factory(trace.clone(), stored.clone(), released, operands.clone()),
                &controller.for_extension(),
                Vec::new(),
            );
            let callback = stored.take().unwrap();
            let retained_factory = callback.clone();
            drop(loaded);
            drop(foreign);
            drop(controller);
            let world = started(directory).await;
            let ui = port(&world.mode);
            ui.set_widget("bound".into(), Some(callback), None).unwrap();
            assert_eq!(*trace.borrow(), ["before", "same", "after", "foreign"]);
            let (terminal, live) = operands.take().unwrap();
            assert!(Rc::ptr_eq(&terminal.children(), &world.mode.ui.children()));
            let before = live.get().unwrap();
            world.themes.set_theme_instance(super::light()).unwrap();
            assert!(!Rc::ptr_eq(&before, &live.get().unwrap()));
            assert!(Rc::ptr_eq(
                &live.get().unwrap(),
                &world.themes.theme().get().unwrap()
            ));
            drop(terminal);
            drop(retained_factory);
            assert!(weak_capture.upgrade().is_some());
            ui.set_widget("bound".into(), None, None).unwrap();
            assert!(weak_capture.upgrade().is_none());
            assert_eq!(
                *trace.borrow(),
                [
                    "before",
                    "same",
                    "after",
                    "foreign",
                    "dispose-before",
                    "same",
                    "dispose-after",
                    "foreign"
                ]
            );
            let weak_mode = Rc::downgrade(&world.mode);
            drop(ui);
            drop(world);
            assert!(weak_mode.upgrade().is_none());
        },
    );
}

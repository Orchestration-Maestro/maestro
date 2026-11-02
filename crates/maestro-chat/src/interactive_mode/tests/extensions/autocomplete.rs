//! Native completion wrappers through the installed extension UI and editor.
use super::super::extension_ui::port;
use super::super::extension_widgets::with_factories;
use super::super::{World, isolated};
use maestro_app::{AutocompleteProviderFactory, ExtensionAPI, Local, NewSessionCommandOptions};
use maestro_tui::autocomplete::{
    CompletionError, CompletionOptions, CompletionResult, CursorPosition,
};
use maestro_tui::{AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, Cancellation};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
};

/// An owned provider supplied to or returned from a factory.
type Provider = Rc<dyn AutocompleteProvider<Signal = Cancellation>>;
/// Calls in their observed outer-to-inner order.
type Trace = Rc<RefCell<Vec<String>>>;

/// A wrapper preserving every borrowed provider operation.
struct Traced {
    /// The provider this wrapper delegates to.
    inner: Provider,
    /// The factory's distinct name.
    name: &'static str,
    /// Every request and application.
    trace: Trace,
}
impl AutocompleteProvider for Traced {
    type Signal = Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a, Cancellation>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>
    {
        self.trace
            .borrow_mut()
            .push(format!("{}:suggest", self.name));
        self.inner.get_suggestions(lines, cursor, options)
    }
    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        self.trace.borrow_mut().push(format!("{}:apply", self.name));
        self.inner.apply_completion(lines, cursor, item, prefix)
    }
    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
    ) -> Option<bool> {
        self.trace
            .borrow_mut()
            .push(format!("{}:trigger", self.name));
        self.inner.should_trigger_file_completion(lines, cursor)
    }
}

/// Binds a wrapper and records the exact provider handles it receives and returns.
fn wrapper(
    api: &ExtensionAPI<Local>,
    name: &'static str,
    trace: Trace,
    handles: Rc<RefCell<Vec<(Provider, Provider)>>>,
) -> AutocompleteProviderFactory {
    api.ui_callback(move |inner: Provider, _| {
        trace.borrow_mut().push(format!("{name}:factory"));
        let outer: Provider = Rc::new(Traced {
            inner: inner.clone(),
            name,
            trace: trace.clone(),
        });
        handles.borrow_mut().push((inner, outer.clone()));
        Ok(outer)
    })
}

/// Completes a unique built-in command through the real editor's installed provider.
fn complete(world: &World) {
    world.mode.current_editor().try_set_text("/sett").unwrap();
    for _ in 0..2 {
        world.terminal.send_input("\t");
        assert!(world.runtime.drain_futures().is_empty());
        world.runtime.settle().unwrap();
    }
    assert_eq!(
        world.mode.current_editor().try_get_text().unwrap(),
        "/settings "
    );
}

/// Checks the complete call sequence, then starts a new observation.
fn trace_is(trace: &Trace, expected: &str) {
    let observed = std::mem::take(&mut *trace.borrow_mut());
    assert_eq!(observed, expected.split_whitespace().collect::<Vec<_>>());
}

/// Checks the optional trigger through the actual wrapped provider.
fn trigger(provider: &Provider) {
    assert_eq!(
        provider
            .should_trigger_file_completion(&["foo".into()], CursorPosition { line: 0, col: 3 }),
        Some(true)
    );
}

#[test]
fn autocomplete_wrappers_stack_over_fresh_base_and_share_the_installed_handle() {
    isolated(
        "autocomplete_wrappers_stack_over_fresh_base_and_share_the_installed_handle",
        async |directory| {
            let trace = Trace::default();
            let handles = Rc::new(RefCell::new(Vec::new()));
            let (seen, captured) = (trace.clone(), handles.clone());
            let knobs = Rc::new(super::super::extension_editors::Knobs::default());
            let supplied = knobs.clone();
            let editor = Rc::new(RefCell::new(None));
            let built = editor.clone();
            let (world, factories) = with_factories(directory, move |api| {
                built.replace(Some(super::super::extension_editors::probe_factory(
                    api, &supplied, true, None,
                )));
                vec![
                    wrapper(api, "A", seen.clone(), captured.clone()),
                    wrapper(api, "B", seen.clone(), captured.clone()),
                ]
            })
            .await;
            complete(&world);
            trace_is(&trace, "");
            let ui = port(&world.mode);
            ui.add_autocomplete_provider(factories[0].clone()).unwrap();
            trace_is(&trace, "A:factory");
            trigger(&handles.borrow()[0].1);
            complete(&world);
            trace_is(&trace, "A:trigger A:suggest A:apply");
            ui.add_autocomplete_provider(factories[1].clone()).unwrap();
            trace_is(&trace, "A:factory B:factory");
            let handles = handles.borrow();
            assert!(
                !Rc::ptr_eq(&handles[0].0, &handles[1].0),
                "a rebuild starts fresh"
            );
            assert!(
                Rc::ptr_eq(&handles[1].1, &handles[2].0),
                "B wraps A's exact result"
            );
            assert!(Rc::ptr_eq(
                world
                    .mode
                    .editor_state
                    .autocomplete
                    .borrow()
                    .as_ref()
                    .unwrap(),
                &handles[2].1
            ));
            trigger(&handles[2].1);
            complete(&world);
            trace_is(
                &trace,
                "B:trigger A:trigger B:suggest A:suggest B:apply A:apply",
            );
            complete_both_editors(&world, editor.take().unwrap(), &knobs, &trace);
        },
    );
}

/// Completes through the custom editor and then the restored default editor.
fn complete_both_editors(
    world: &World,
    factory: maestro_app::EditorFactory,
    knobs: &super::super::extension_editors::Knobs,
    trace: &Trace,
) {
    let ui = port(&world.mode);
    ui.set_editor_component(Some(factory)).unwrap();
    complete(world);
    assert_eq!(knobs.editor().inner.get_text(), "/settings ");
    trace_is(trace, "B:suggest A:suggest B:apply A:apply");
    ui.set_editor_component(None).unwrap();
    complete(world);
    trace_is(trace, "B:suggest A:suggest B:apply A:apply");
}

#[test]
fn wrapper_factories_stack_over_a_fresh_base_on_both_installed_editors() {
    isolated(
        "wrapper_factories_stack_over_a_fresh_base_on_both_installed_editors",
        async |directory| {
            let trace = Trace::default();
            let handles = Rc::new(RefCell::new(Vec::new()));
            let knobs = Rc::new(super::super::extension_editors::Knobs::default());
            let (seen, captured, supplied) = (trace.clone(), handles.clone(), knobs.clone());
            let (world, mut built) = with_factories(directory, move |api| {
                vec![(
                    super::super::extension_editors::probe_factory(api, &supplied, true, None),
                    [
                        wrapper(api, "A", seen.clone(), captured.clone()),
                        wrapper(api, "B", seen.clone(), captured.clone()),
                    ],
                )]
            })
            .await;
            let (editor, [first, second]) = built.pop().unwrap();
            let ui = port(&world.mode);
            ui.set_editor_component(Some(editor)).unwrap();
            knobs.calls.take();
            ui.add_autocomplete_provider(first).unwrap();
            ui.add_autocomplete_provider(second).unwrap();
            trace_is(&trace, "A:factory A:factory B:factory");
            assert_eq!(knobs.calls.take(), ["provider", "provider"]);
            let handles = handles.borrow();
            assert!(
                !Rc::ptr_eq(&handles[0].0, &handles[1].0),
                "a rebuild starts fresh"
            );
            assert!(
                Rc::ptr_eq(&handles[1].1, &handles[2].0),
                "B wraps A's exact result"
            );
            let installed = world
                .mode
                .editor_state
                .autocomplete
                .borrow()
                .clone()
                .unwrap();
            assert!(Rc::ptr_eq(&installed, &handles[2].1));
            assert!(Rc::ptr_eq(
                knobs.provider.borrow().as_ref().unwrap(),
                &installed
            ));
            trigger(&installed);
            trace_is(&trace, "B:trigger A:trigger");
            complete(&world);
            assert_eq!(knobs.editor().inner.get_text(), "/settings ");
            trace_is(&trace, "B:suggest A:suggest B:apply A:apply");
            ui.set_editor_component(None).unwrap();
            assert!(knobs.calls.borrow().iter().all(|call| call != "provider"));
            complete(&world);
            trace_is(&trace, "B:suggest A:suggest B:apply A:apply");
        },
    );
}

#[test]
fn session_replacement_drops_wrappers_and_installs_a_fresh_base() {
    isolated(
        "session_replacement_drops_wrappers_and_installs_a_fresh_base",
        async |directory| {
            let trace = Trace::default();
            let handles = Rc::new(RefCell::new(Vec::new()));
            let (seen, captured) = (trace.clone(), handles.clone());
            let (world, factories) = with_factories(directory, move |api| {
                vec![wrapper(api, "A", seen.clone(), captured.clone())]
            })
            .await;
            port(&world.mode)
                .add_autocomplete_provider(factories[0].clone())
                .unwrap();
            complete(&world);
            trace_is(&trace, "A:factory A:suggest A:apply");
            world
                .host
                .new_session(NewSessionCommandOptions::default())
                .await
                .unwrap();
            trace_is(&trace, "");
            let installed = world
                .mode
                .editor_state
                .autocomplete
                .borrow()
                .clone()
                .unwrap();
            let handles = handles.borrow();
            assert!(
                !Rc::ptr_eq(&installed, &handles[0].0),
                "the base is rebuilt"
            );
            assert!(!Rc::ptr_eq(&installed, &handles[0].1));
            complete(&world);
            trace_is(&trace, "");
        },
    );
}

/// Appends one factory during its first invocation and observes the nested installation.
fn append_once(
    api: &ExtensionAPI<Local>,
    trace: Trace,
    host: Rc<RefCell<Weak<World>>>,
) -> AutocompleteProviderFactory {
    let next = wrapper(api, "B", trace.clone(), Rc::default());
    let appended = Cell::new(false);
    api.ui_callback(move |inner: Provider, _| {
        trace.borrow_mut().push("A:factory".into());
        if !appended.replace(true) {
            let world = host.borrow().upgrade().unwrap();
            port(&world.mode).add_autocomplete_provider(next.clone())?;
            complete(&world);
        }
        Ok(Rc::new(Traced {
            inner,
            name: "A",
            trace: trace.clone(),
        }) as Provider)
    })
}

#[test]
fn autocomplete_factories_retain_origin_and_rebuild_after_each_registration() {
    isolated(
        "autocomplete_factories_retain_origin_and_rebuild_after_each_registration",
        async |directory| {
            let trace = Trace::default();
            let slot: Rc<RefCell<Weak<World>>> = Rc::default();
            let (seen, host) = (trace.clone(), slot.clone());
            let (world, factories) = with_factories(directory, move |api| {
                vec![append_once(api, seen.clone(), host.clone())]
            })
            .await;
            let world = Rc::new(world);
            slot.replace(Rc::downgrade(&world));
            let ui = port(&world.mode);
            ui.add_autocomplete_provider(factories[0].clone()).unwrap();
            trace_is(
                &trace,
                "A:factory A:factory B:factory B:suggest A:suggest B:apply A:apply B:factory",
            );
            complete(&world);
            trace_is(&trace, "B:suggest A:suggest B:apply A:apply");
            ui.add_autocomplete_provider(factories[0].clone()).unwrap();
            trace_is(&trace, "A:factory B:factory A:factory");
            complete(&world);
            trace_is(
                &trace,
                "A:suggest B:suggest A:suggest A:apply B:apply A:apply",
            );
        },
    );
}

/// A factory that can fail with an identifiable native error, then recover.
fn failing(
    api: &ExtensionAPI<Local>,
    trace: Trace,
    refused: Rc<Cell<bool>>,
    address: Rc<Cell<usize>>,
) -> AutocompleteProviderFactory {
    api.ui_callback(move |inner: Provider, _| {
        trace.borrow_mut().push("F:factory".into());
        if refused.get() {
            let error: maestro_app::EventError = Box::new(std::io::Error::from_raw_os_error(13));
            address.set(std::ptr::from_ref(&*error).cast::<()>().addr());
            return Err(error);
        }
        Ok(inner)
    })
}

/// Proves that the native error allocation and its concrete kind survived forwarding.
fn original(error: &(dyn std::error::Error + 'static), address: usize) {
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .raw_os_error(),
        Some(13)
    );
    assert_eq!(std::ptr::from_ref(error).cast::<()>().addr(), address);
}

/// Checks binding and settings forwarding, including the saved preference on failure.
async fn failed_consumers(world: &World, address: &Cell<usize>) {
    let error = world.mode.rebind_current_session().await.unwrap_err();
    original(&*error, address.get());
    let callbacks = world.mode.settings_callbacks(Rc::new(|| {}));
    (callbacks.on_enable_skill_commands_change)(false);
    let errors = world.runtime.drain_futures();
    assert_eq!(errors.len(), 1);
    original(&*errors[0], address.get());
    assert!(
        !world
            .host
            .session()
            .settings_manager()
            .borrow()
            .get_enable_skill_commands()
    );
}

/// Rebuilds successfully after a failure without removing any registration.
fn recovered(world: &World, trace: &Trace, safe_before: bool) {
    let callbacks = world.mode.settings_callbacks(Rc::new(|| {}));
    (callbacks.on_enable_skill_commands_change)(true);
    assert!(world.runtime.drain_futures().is_empty());
    trace_is(
        trace,
        if safe_before {
            "A:factory F:factory L:factory"
        } else {
            "F:factory L:factory"
        },
    );
    complete(world);
    trace_is(
        trace,
        if safe_before {
            "L:suggest A:suggest L:apply A:apply"
        } else {
            "L:suggest L:apply"
        },
    );
}

#[test]
fn wrapper_failure_keeps_the_previous_provider_and_original_error() {
    isolated(
        "wrapper_failure_keeps_the_previous_provider_and_original_error",
        async |directory| {
            for safe_before in [false, true] {
                let trace = Trace::default();
                let refused = Rc::new(Cell::new(true));
                let address = Rc::new(Cell::new(0));
                let (seen, fail, captured) = (trace.clone(), refused.clone(), address.clone());
                let (world, factories) = with_factories(directory, move |api| {
                    vec![
                        wrapper(api, "A", seen.clone(), Rc::default()),
                        failing(api, seen.clone(), fail.clone(), captured.clone()),
                        wrapper(api, "L", seen.clone(), Rc::default()),
                    ]
                })
                .await;
                let ui = port(&world.mode);
                if safe_before {
                    ui.add_autocomplete_provider(factories[0].clone()).unwrap();
                }
                trace.borrow_mut().clear();
                let error = ui
                    .add_autocomplete_provider(factories[1].clone())
                    .unwrap_err();
                original(&*error, address.get());
                trace_is(
                    &trace,
                    if safe_before {
                        "A:factory F:factory"
                    } else {
                        "F:factory"
                    },
                );
                complete(&world);
                trace_is(&trace, if safe_before { "A:suggest A:apply" } else { "" });
                let error = ui
                    .add_autocomplete_provider(factories[2].clone())
                    .unwrap_err();
                original(&*error, address.get());
                failed_consumers(&world, &address).await;
                assert!(!trace.borrow().iter().any(|entry| entry == "L:factory"));
                refused.set(false);
                trace.borrow_mut().clear();
                recovered(&world, &trace, safe_before);
            }
        },
    );
}

/// One listener observing the factory's originating callback frame.
fn observe(bus: &maestro_app::EventBus<Local>, trace: Trace, label: &'static str) {
    bus.on(
        "completion",
        Rc::new(move |_, _| {
            trace.borrow_mut().push(label.into());
            Ok(None)
        }),
    );
}

#[test]
fn autocomplete_factories_keep_origin_after_transfer_and_facade_release() {
    isolated(
        "autocomplete_factories_keep_origin_after_transfer_and_facade_release",
        async |directory| {
            let trace = Trace::default();
            let controller =
                maestro_app::create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}));
            let foreign = controller.for_extension();
            observe(&foreign, trace.clone(), "foreign");
            let stored = Rc::new(RefCell::new(None));
            let output = stored.clone();
            let recorded = trace.clone();
            let retained = Rc::new(());
            let released = Rc::downgrade(&retained);
            let factory: maestro_app::ExtensionFactory<Local> = Rc::new(move |api| {
                observe(&api.events, recorded.clone(), "same");
                let (recorded, retained) = (recorded.clone(), retained.clone());
                output.replace(Some(api.ui_callback(move |inner: Provider, emitter| {
                    let _ = &retained;
                    recorded.borrow_mut().push("before".into());
                    emitter.emit("completion", serde_json::Value::Null);
                    assert_eq!(*recorded.borrow(), ["before", "same"]);
                    recorded.borrow_mut().push("after".into());
                    Ok(inner)
                })));
                Box::pin(std::future::ready(Ok(())))
            });
            let loaded = super::super::loaded_on_bus(
                directory.to_str().unwrap(),
                factory,
                &controller.for_extension(),
                Vec::new(),
            );
            let callback = stored.take().unwrap();
            drop((loaded, foreign, controller));
            let world = super::super::started(directory).await;
            port(&world.mode)
                .add_autocomplete_provider(callback)
                .unwrap();
            trace_is(&trace, "before same after foreign");
            complete(&world);
            assert!(
                released.upgrade().is_some(),
                "registration owns the bound callback"
            );
            let mode = Rc::downgrade(&world.mode);
            drop(world);
            assert!(
                mode.upgrade().is_none(),
                "registration does not retain the mode"
            );
            assert!(
                released.upgrade().is_none(),
                "dropping the mode releases the factory"
            );
        },
    );
}

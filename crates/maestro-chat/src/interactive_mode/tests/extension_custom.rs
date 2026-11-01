//! Extension custom views shown inline or as overlays through the retained host.
use super::extension_editors::{Knobs, probe_factory};
use super::extension_ui::port;
use super::extension_widgets::with_factories;
use super::{World, isolated};
use maestro_app::{
    CustomFactoryOutcome, CustomUIOptions, EditorFactory, EventError, ExtensionAPI,
    ExtensionComponent, ExtensionKeybindings, Local, OverlayOptionsSource, UIHostCall,
};
use maestro_theme::LiveTheme;
use maestro_tui::tui::InputHandler;
use maestro_tui::{
    Component, EditorComponent, FocusFlag, Focusable, OverlayHandle, OverlayOptions, SizeValue, TUI,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
};
use tokio::sync::oneshot;

/// Completes a typed custom view.
pub(super) type Done = Rc<dyn Fn(String) -> Result<(), EventError>>;

/// The typed custom-view factory the witnesses build.
pub(super) type ViewFactory =
    UIHostCall<(TUI, LiveTheme, ExtensionKeybindings, Done), CustomFactoryOutcome>;

/// The overlay handle an extension receives.
pub(super) type Handle = Rc<RefCell<Box<dyn OverlayHandle>>>;

/// A focusable view recording the width it renders at.
pub(super) struct View {
    /// The painted label.
    label: &'static str,
    /// The view's focus.
    focus: FocusFlag,
    /// The last width the view rendered at.
    width: Cell<Option<usize>>,
    /// Raw input delivered by the retained terminal host.
    input: RefCell<Vec<String>>,
}
impl Focusable for View {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl InputHandler for View {
    fn handle_input(&self, data: &str) {
        self.input.borrow_mut().push(data.to_owned());
    }
}
impl Component for View {
    fn render(&self, width: usize) -> Vec<String> {
        self.width.set(Some(width));
        vec![self.label.to_owned()]
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

/// When and how a witness's factory settles its view.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Flow {
    /// Returns the component at once.
    Ready,
    /// Settles the component on the terminal runtime.
    Async,
    /// Fails at once.
    FailReady,
    /// Fails on the terminal runtime.
    FailAsync,
    /// Calls `done` with `first`, then returns the component at once.
    DoneFirst,
    /// Calls `done` inside the delayed build, before returning its component.
    AsyncDone,
}

/// How a witness's factory builds its view.
#[derive(Clone, Copy)]
pub(super) struct Build {
    /// When and how the view settles.
    flow: Flow,
    /// Fails the component's disposal.
    dispose_fails: bool,
    /// The component's own width metadata.
    width: Option<f64>,
}

/// A view settling as `flow` selects, with no width.
pub(super) const fn plain(flow: Flow) -> Build {
    Build {
        flow,
        dispose_fails: false,
        width: None,
    }
}

/// What the overlay resolver or handle callback does when called.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Setup {
    /// Returns or accepts normally.
    Normal,
    /// Fails with the witness's error.
    Throw,
    /// Completes the view with `first`.
    Done,
}

/// What the witnesses observe of the views their factories build.
pub(super) struct Views {
    /// Effects in call order.
    pub(super) trace: RefCell<Vec<String>>,
    /// The completion each factory call received.
    done: RefCell<Vec<Done>>,
    /// The views built, kept weakly in build order.
    pub(super) built: RefCell<Vec<Weak<View>>>,
    /// The identity the failures carry.
    error: Arc<()>,
    /// Releases asynchronous builds, in order.
    gates: RefCell<Vec<oneshot::Sender<()>>>,
    /// What the resolver does.
    resolve: Cell<Setup>,
    /// What the handle callback does.
    on_handle: Cell<Setup>,
    /// The options the resolver returns.
    options: Rc<RefCell<OverlayOptions>>,
    /// The handles the handle callback received.
    handles: RefCell<Vec<Handle>>,
    /// Original close failures caught inside a delayed build.
    close_failures: RefCell<Vec<EventError>>,
}

/// The callbacks one witness binds to its extension.
pub(super) struct Bound {
    /// The view factories, by label.
    pub(super) views: Vec<ViewFactory>,
    /// The overlay options resolver.
    pub(super) resolver: UIHostCall<(), Rc<RefCell<OverlayOptions>>>,
    /// The overlay handle callback.
    pub(super) on_handle: UIHostCall<Handle, ()>,
}

/// An error whose allocation identity survives host forwarding.
#[derive(Debug)]
struct Original(Arc<()>);
impl std::fmt::Display for Original {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("custom view failed")
    }
}
impl std::error::Error for Original {}

impl Views {
    /// A fresh witness whose resolver returns a width of 21.
    pub(super) fn new() -> Rc<Self> {
        Rc::new(Self {
            trace: RefCell::default(),
            done: RefCell::default(),
            built: RefCell::default(),
            error: Arc::new(()),
            gates: RefCell::default(),
            resolve: Cell::new(Setup::Normal),
            on_handle: Cell::new(Setup::Normal),
            options: width_options(21),
            handles: RefCell::default(),
            close_failures: RefCell::default(),
        })
    }
    /// Records `effect`.
    fn mark(&self, effect: impl Into<String>) {
        self.trace.borrow_mut().push(effect.into());
    }
    /// The witness's original failure.
    fn failure(&self) -> EventError {
        Box::new(Original(self.error.clone()))
    }
    /// Whether `error` carries this witness's original failure.
    fn is_original(&self, error: &(dyn std::error::Error + 'static)) -> bool {
        error
            .downcast_ref::<Original>()
            .is_some_and(|error| Arc::ptr_eq(&error.0, &self.error))
    }
    /// The `index`th view built, while it is alive.
    fn view(&self, index: usize) -> Rc<View> {
        self.built.borrow()[index].upgrade().unwrap()
    }
    /// Calls the `index`th factory call's completion with `value`.
    pub(super) fn complete(&self, index: usize, value: &str) -> Result<(), EventError> {
        let done = self.done.borrow()[index].clone();
        done(value.into())
    }
    /// The component for `label`, with traced disposal.
    fn component(
        self: &Rc<Self>,
        api: &ExtensionAPI<Local>,
        label: &'static str,
        build: Build,
    ) -> ExtensionComponent {
        let view = Rc::new(View {
            label,
            focus: FocusFlag::default(),
            width: Cell::new(None),
            input: RefCell::default(),
        });
        self.built.borrow_mut().push(Rc::downgrade(&view));
        let views = self.clone();
        ExtensionComponent {
            component: view,
            dispose: Some(api.ui_callback(move |(), _| {
                views.mark(format!("{label}:dispose"));
                if build.dispose_fails {
                    return Err(views.failure());
                }
                Ok(())
            })),
            set_expanded: None,
            width: build.width,
        }
    }
    /// A factory building `label` as `build` selects.
    fn factory(
        self: &Rc<Self>,
        api: &ExtensionAPI<Local>,
        label: &'static str,
        build: Build,
    ) -> ViewFactory {
        let (views, api) = (self.clone(), api.clone());
        api.clone().ui_callback(
            move |(_, _, _, done): (TUI, LiveTheme, ExtensionKeybindings, Done), _| {
                views.done.borrow_mut().push(done.clone());
                match build.flow {
                    Flow::DoneFirst => done("first".into())?,
                    Flow::FailReady => return Err(views.failure()),
                    _ => {}
                }
                let component = views.component(&api, label, build);
                if matches!(build.flow, Flow::Ready | Flow::DoneFirst) {
                    return Ok(CustomFactoryOutcome::Ready(component));
                }
                let (gate, opened) = oneshot::channel();
                views.gates.borrow_mut().push(gate);
                let failure = (build.flow == Flow::FailAsync).then(|| views.failure());
                let views = views.clone();
                Ok(CustomFactoryOutcome::Async(Box::pin(async move {
                    opened.await.unwrap();
                    let close = (build.flow == Flow::AsyncDone).then(|| done("first".into()));
                    views
                        .close_failures
                        .borrow_mut()
                        .extend(close.into_iter().filter_map(Result::err));
                    match failure {
                        Some(failure) => Err(failure),
                        None => Ok(component),
                    }
                })))
            },
        )
    }
    /// The views `builds` describe with the resolver and handle callback.
    pub(super) fn bind(
        self: &Rc<Self>,
        api: &ExtensionAPI<Local>,
        builds: &[(&'static str, Build)],
    ) -> Bound {
        let (resolving, handling) = (self.clone(), self.clone());
        Bound {
            views: builds
                .iter()
                .map(|(label, build)| self.factory(api, label, *build))
                .collect(),
            resolver: api.ui_callback(move |(), _| {
                resolving.mark("resolve-options");
                match resolving.resolve.get() {
                    Setup::Throw => return Err(resolving.failure()),
                    Setup::Done => resolving.complete(0, "first")?,
                    Setup::Normal => {}
                }
                Ok(resolving.options.clone())
            }),
            on_handle: api.ui_callback(move |handle: Handle, _| {
                handling.mark("on-handle");
                handling.handles.borrow_mut().push(handle);
                match handling.on_handle.get() {
                    Setup::Throw => Err(handling.failure()),
                    Setup::Done => handling.complete(0, "first"),
                    Setup::Normal => Ok(()),
                }
            }),
        }
    }
    /// Hides and shows again each overlay through the handle the extension received.
    fn toggle_handles(&self) {
        for handle in self.handles.borrow().iter() {
            handle.borrow_mut().set_hidden(true);
            assert!(handle.borrow().is_hidden());
            handle.borrow_mut().set_hidden(false);
        }
    }
    /// Releases the next asynchronous build and runs the host's admitted work.
    pub(super) fn release(&self, world: &World) {
        let gate = self.gates.borrow_mut().remove(0);
        gate.send(()).unwrap();
        assert!(world.runtime.drain_futures().is_empty());
    }
}

/// Starts a world whose extension binds `builds` on `views`.
async fn started(
    directory: &std::path::Path,
    views: &Rc<Views>,
    builds: &'static [(&'static str, Build)],
) -> (World, Bound) {
    let witness = views.clone();
    let (world, mut bound) =
        with_factories(directory, move |api| vec![witness.bind(api, builds)]).await;
    world.mode.editor.set_text("saved");
    (world, bound.pop().unwrap())
}

/// Options showing the view as an overlay with `source` and `on_handle`.
pub(super) fn overlay(
    source: Option<OverlayOptionsSource>,
    on_handle: Option<UIHostCall<Handle, ()>>,
) -> CustomUIOptions {
    CustomUIOptions {
        overlay: Some(true),
        overlay_options: source,
        on_handle,
    }
}

/// Options with a static width of `cells`.
fn width_options(cells: isize) -> Rc<RefCell<OverlayOptions>> {
    Rc::new(RefCell::new(OverlayOptions {
        width: Some(SizeValue::Cells(cells)),
        ..OverlayOptions::default()
    }))
}

/// Whether the editor container shows the current editor with `text`.
fn restored(world: &World, text: &str) -> bool {
    let editor = world.mode.current_editor();
    let shown = world.mode.editor_container.children().borrow()[0].clone();
    std::ptr::addr_eq(Rc::as_ptr(&shown), Rc::as_ptr(&editor)) && editor.get_text() == text
}

/// One synchronous and one asynchronous view.
const BOTH: &[(&str, Build)] = &[
    (
        "CUSTOM",
        Build {
            flow: Flow::Ready,
            dispose_fails: false,
            width: Some(17.0),
        },
    ),
    (
        "CUSTOM",
        Build {
            flow: Flow::Async,
            dispose_fails: false,
            width: Some(17.0),
        },
    ),
];

/// Shows the `index`th view of `BOTH` inline, or as an overlay with the component width
/// (`source` 0), static options (1) or resolved options (2), then completes it twice.
async fn installs_and_closes(
    directory: &std::path::Path,
    index: usize,
    overlaid: bool,
    source: u8,
) {
    let views = Views::new();
    let (world, bound) = started(directory, &views, BOTH).await;
    let options = overlaid.then(|| match source {
        0 => overlay(None, Some(bound.on_handle.clone())),
        1 => overlay(
            Some(OverlayOptionsSource::Static(views.options.clone())),
            None,
        ),
        _ => overlay(
            Some(OverlayOptionsSource::Resolver(bound.resolver.clone())),
            None,
        ),
    });
    let result = port(&world.mode).custom(bound.views[index].clone(), options);
    if index == 1 {
        views.release(&world);
    }
    world.runtime.settle().unwrap();
    let view = views.view(0);
    assert!(view.focus.get());
    assert_eq!(world.mode.ui.has_overlay(), overlaid);
    let expected = match (overlaid, source) {
        (false, _) => 80,
        (true, 0) => 17,
        _ => 21,
    };
    assert_eq!(view.width.get(), Some(expected));
    views.toggle_handles();
    world.runtime.settle().unwrap();
    let resolved = views
        .trace
        .borrow()
        .iter()
        .filter(|e| *e == "resolve-options")
        .count();
    assert_eq!(resolved, usize::from(overlaid && source == 2));
    views.complete(0, "first").unwrap();
    views.complete(0, "second").unwrap();
    assert_eq!(result.await.unwrap().as_deref(), Some("first"));
    assert!(!world.mode.ui.has_overlay());
    assert!(restored(&world, "saved"));
    assert_eq!(views.trace.borrow().last().unwrap(), "CUSTOM:dispose");
}

#[test]
fn custom_views_install_close_and_keep_overlay_options() {
    isolated(
        "custom_views_install_close_and_keep_overlay_options",
        async |directory| {
            for index in 0..2 {
                for (overlaid, source) in [(false, 0), (true, 0), (true, 1), (true, 2)] {
                    installs_and_closes(directory, index, overlaid, source).await;
                }
            }
        },
    );
}

/// Two overlay views A and B.
const TWO: &[(&str, Build)] = &[("A", plain(Flow::Ready)), ("B", plain(Flow::Ready))];

#[test]
fn custom_overlays_close_their_own_view() {
    isolated("custom_overlays_close_their_own_view", async |directory| {
        for older_first in [true, false] {
            let views = Views::new();
            let (world, bound) = started(directory, &views, TWO).await;
            let ui = port(&world.mode);
            let first = ui.custom(bound.views[0].clone(), Some(overlay(None, None)));
            let second = ui.custom(bound.views[1].clone(), Some(overlay(None, None)));
            let (closing, surviving) = if older_first { (0, 1) } else { (1, 0) };
            views.complete(closing, "first").unwrap();
            assert!(world.mode.ui.has_overlay());
            assert!(views.view(surviving).focus.get());
            let label = ["A", "B"][closing];
            assert_eq!(*views.trace.borrow(), [format!("{label}:dispose")]);
            views.complete(surviving, "second").unwrap();
            assert!(!world.mode.ui.has_overlay());
            let results = [first.await.unwrap(), second.await.unwrap()];
            let expected = if older_first {
                ["first", "second"]
            } else {
                ["second", "first"]
            };
            assert_eq!(results.map(Option::unwrap), expected);
        }
    });
}

/// Failing synchronous and asynchronous builds.
const FAILING: &[(&str, Build)] = &[("C", plain(Flow::FailReady)), ("C", plain(Flow::FailAsync))];

#[test]
fn custom_factory_failures_keep_original_error() {
    isolated(
        "custom_factory_failures_keep_original_error",
        async |directory| {
            for (index, overlaid) in [(0, false), (0, true), (1, false), (1, true)] {
                let views = Views::new();
                let (world, bound) = started(directory, &views, FAILING).await;
                let options = overlaid.then(|| overlay(None, None));
                let result = port(&world.mode).custom(bound.views[index].clone(), options);
                if index == 1 {
                    views.release(&world);
                }
                let error = result.await.map(drop).unwrap_err();
                assert!(views.is_original(error.as_ref()));
                assert!(restored(&world, "saved"));
                assert!(!world.mode.ui.has_overlay());
                assert!(views.trace.borrow().is_empty());
            }
        },
    );
}

#[test]
fn custom_inline_restore_failure_keeps_its_original_error() {
    isolated(
        "custom_inline_restore_failure_keeps_its_original_error",
        async |directory| {
            let (views, knobs) = (Views::new(), Rc::new(Knobs::default()));
            let (witness, captured) = (views.clone(), knobs.clone());
            let (world, mut bound) = with_factories(directory, move |api| {
                let editor: EditorFactory = probe_factory(api, &captured, true, None);
                vec![(witness.bind(api, FAILING), editor)]
            })
            .await;
            let (bound, editor) = bound.pop().unwrap();
            let ui = port(&world.mode);
            ui.set_editor_component(Some(editor)).unwrap();
            knobs.fail.set(Some("set_text"));
            let result = ui.custom(bound.views[1].clone(), None);
            views.release(&world);
            let error = result.await.map(drop).unwrap_err();
            assert!(knobs.is_original(error.as_ref()));
        },
    );
}

/// Views completing before their build returns.
const EARLY: &[(&str, Build)] = &[("C", plain(Flow::DoneFirst)), ("C", plain(Flow::AsyncDone))];

#[test]
fn custom_done_before_factory_completion_does_not_install() {
    isolated(
        "custom_done_before_factory_completion_does_not_install",
        async |directory| {
            for index in 0..2 {
                let views = Views::new();
                let (world, bound) = started(directory, &views, EARLY).await;
                let result = port(&world.mode).custom(bound.views[index].clone(), None);
                if index == 1 {
                    views.release(&world);
                }
                assert_eq!(result.await.unwrap().as_deref(), Some("first"));
                assert!(restored(&world, "saved"));
                assert_eq!(*views.trace.borrow(), ["C:dispose"]);
                assert!(views.built.borrow()[0].upgrade().is_none());
            }
        },
    );
}

#[test]
fn custom_async_restore_failure_settles_with_original_error() {
    isolated(
        "custom_async_restore_failure_settles_with_original_error",
        async |directory| {
            let (views, knobs) = (Views::new(), Rc::new(Knobs::default()));
            let (witness, captured) = (views.clone(), knobs.clone());
            let (world, mut bound) = with_factories(directory, move |api| {
                let editor: EditorFactory = probe_factory(api, &captured, true, None);
                vec![(witness.bind(api, EARLY), editor)]
            })
            .await;
            let (bound, editor) = bound.pop().unwrap();
            let ui = port(&world.mode);
            ui.set_editor_component(Some(editor)).unwrap();
            let mut result = ui.custom(bound.views[1].clone(), None);
            knobs.calls.take();
            knobs.fail.set(Some("set_text"));
            views.release(&world);
            let first = views.close_failures.borrow_mut().pop().unwrap();
            let repeated = views.complete(0, "again").unwrap_err();
            let observed = super::typed::poll(&mut result)
                .expect("a failed restoration settles the observation")
                .map(drop)
                .unwrap_err();
            let sources = [&first, &repeated, &observed].map(|error| error.source().unwrap());
            assert!(sources.iter().all(|source| knobs.is_original(*source)));
            let original = sources.map(|source| std::ptr::from_ref(source).cast::<()>());
            assert!(original.iter().all(|address| *address == original[0]));
            assert_eq!(*knobs.calls.borrow(), ["set_text"]);
        },
    );
}

#[test]
fn custom_overlay_callbacks_keep_original_failures_and_close_during_setup() {
    isolated(
        "custom_overlay_callbacks_keep_original_failures_and_close_during_setup",
        async |directory| {
            for (resolve, on_handle) in [
                (Setup::Throw, Setup::Normal),
                (Setup::Done, Setup::Normal),
                (Setup::Normal, Setup::Throw),
                (Setup::Normal, Setup::Done),
            ] {
                let views = Views::new();
                let (world, bound) = started(directory, &views, BOTH).await;
                views.resolve.set(resolve);
                views.on_handle.set(on_handle);
                let source = OverlayOptionsSource::Resolver(bound.resolver.clone());
                let options = Some(overlay(Some(source), Some(bound.on_handle.clone())));
                let result = port(&world.mode)
                    .custom(bound.views[0].clone(), options)
                    .await;
                assert!(!world.mode.ui.has_overlay());
                let disposed = views
                    .trace
                    .borrow()
                    .iter()
                    .filter(|e| *e == "CUSTOM:dispose")
                    .count();
                if Setup::Throw == resolve || Setup::Throw == on_handle {
                    assert!(views.is_original(result.map(drop).unwrap_err().as_ref()));
                    assert_eq!(disposed, usize::from(on_handle == Setup::Throw));
                } else {
                    assert_eq!(result.unwrap().as_deref(), Some("first"));
                    assert_eq!(disposed, 1);
                }
            }
        },
    );
}

/// One overlay view for each component width.
const WIDTHS: &[(&str, Build)] = &[
    ("C", plain(Flow::Ready)),
    (
        "C",
        Build {
            flow: Flow::Ready,
            dispose_fails: false,
            width: Some(0.0),
        },
    ),
    (
        "C",
        Build {
            flow: Flow::Ready,
            dispose_fails: false,
            width: Some(-3.0),
        },
    ),
    (
        "C",
        Build {
            flow: Flow::Ready,
            dispose_fails: false,
            width: Some(17.0),
        },
    ),
    (
        "C",
        Build {
            flow: Flow::Ready,
            dispose_fails: false,
            width: Some(0.3),
        },
    ),
    (
        "C",
        Build {
            flow: Flow::Ready,
            dispose_fails: false,
            width: Some(19.5),
        },
    ),
];

#[test]
fn custom_overlay_width_uses_explicit_options_before_component_fallback() {
    isolated(
        "custom_overlay_width_uses_explicit_options_before_component_fallback",
        async |directory| {
            let views = Views::new();
            let (world, bound) = started(directory, &views, WIDTHS).await;
            let ui = port(&world.mode);
            for (index, painted) in [80, 80, 1, 17, 1, 19].into_iter().enumerate() {
                let result = ui.custom(bound.views[index].clone(), Some(overlay(None, None)));
                world.runtime.settle().unwrap();
                assert_eq!(views.view(index).width.get(), Some(painted), "{index}");
                views.complete(index, "done").unwrap();
                result.await.unwrap();
            }
            let empty = Rc::new(RefCell::new(OverlayOptions::default()));
            for (source, options, before, after) in [
                (OverlayOptionsSource::Static(empty.clone()), empty, 80, 31),
                (
                    OverlayOptionsSource::Static(views.options.clone()),
                    views.options.clone(),
                    21,
                    31,
                ),
                (
                    OverlayOptionsSource::Resolver(bound.resolver.clone()),
                    views.options.clone(),
                    31,
                    41,
                ),
            ] {
                let index = views.done.borrow().len();
                let result = ui.custom(bound.views[3].clone(), Some(overlay(Some(source), None)));
                world.runtime.settle().unwrap();
                assert_eq!(views.view(index).width.get(), Some(before));
                options.borrow_mut().width = Some(SizeValue::Cells(after));
                world.mode.ui.request_render(false);
                world.runtime.settle().unwrap();
                assert_eq!(
                    views.view(index).width.get(),
                    Some(usize::try_from(after).unwrap())
                );
                views.complete(index, "done").unwrap();
                result.await.unwrap();
            }
        },
    );
}

/// Views whose disposal fails.
const DISPOSE_FAILS: &[(&str, Build)] = &[
    (
        "C",
        Build {
            flow: Flow::Ready,
            dispose_fails: true,
            width: None,
        },
    ),
    (
        "C",
        Build {
            flow: Flow::Async,
            dispose_fails: true,
            width: None,
        },
    ),
];

#[test]
fn custom_disposal_failure_never_replaces_a_committed_result() {
    isolated(
        "custom_disposal_failure_never_replaces_a_committed_result",
        async |directory| {
            for index in 0..2 {
                let views = Views::new();
                let (world, bound) = started(directory, &views, DISPOSE_FAILS).await;
                let result = port(&world.mode).custom(bound.views[index].clone(), None);
                if index == 1 {
                    views.release(&world);
                }
                views.complete(0, "kept").unwrap();
                world.mode.editor.set_text("later");
                views.complete(0, "again").unwrap();
                assert_eq!(result.await.unwrap().as_deref(), Some("kept"));
                assert!(restored(&world, "later"));
                assert_eq!(*views.trace.borrow(), ["C:dispose"]);
            }
        },
    );
}

#[test]
fn custom_view_observer_drop_keeps_admitted_work_alive() {
    isolated(
        "custom_view_observer_drop_keeps_admitted_work_alive",
        async |directory| {
            let views = Views::new();
            let (world, bound) = started(directory, &views, BOTH).await;
            drop(port(&world.mode).custom(bound.views[1].clone(), None));
            views.release(&world);
            let view = views.view(0);
            assert!(view.focus.get());
            world.terminal.send_input("x");
            assert_eq!(*view.input.borrow(), ["x"]);
            views.complete(0, "first").unwrap();
            assert!(restored(&world, "saved"));
            assert_eq!(*views.trace.borrow(), ["CUSTOM:dispose"]);
            drop((view, bound));
            assert!(views.built.borrow()[0].upgrade().is_none());
            assert!(world.mode.custom_views.borrow_mut().snapshot().is_empty());
            let weak = Rc::downgrade(&world.mode);
            drop(world);
            assert!(weak.upgrade().is_none());
        },
    );
}

#[test]
fn custom_overlay_settlement_releases_callback_state() {
    isolated(
        "custom_overlay_settlement_releases_callback_state",
        async |directory| {
            for (index, reset) in [(0, false), (0, true), (1, true)] {
                let views = Views::new();
                let weak = Rc::downgrade(&views);
                let building = weak.clone();
                let (world, mut bound) = with_factories(directory, move |api| {
                    vec![building.upgrade().unwrap().bind(api, BOTH)]
                })
                .await;
                let bound = bound.pop().unwrap();
                let options = overlay(
                    Some(OverlayOptionsSource::Resolver(bound.resolver.clone())),
                    Some(bound.on_handle.clone()),
                );
                let result = port(&world.mode).custom(bound.views[index].clone(), Some(options));
                if reset {
                    world.mode.reset_extension_ui().unwrap();
                } else {
                    views.complete(0, "done").unwrap();
                }
                if index == 1 {
                    views.release(&world);
                }
                assert_eq!(result.await.unwrap().as_deref(), (!reset).then_some("done"));
                assert!(!world.mode.ui.has_overlay());
                let expected = if index == 0 {
                    vec!["resolve-options", "on-handle", "CUSTOM:dispose"]
                } else {
                    vec!["CUSTOM:dispose"]
                };
                assert_eq!(*views.trace.borrow(), expected);
                drop((bound, views));
                assert!(
                    weak.upgrade().is_none(),
                    "settled options must release callbacks"
                );
            }
        },
    );
}

#[test]
fn custom_overlay_visibility_completion_removes_only_its_own_view() {
    isolated(
        "custom_overlay_visibility_completion_removes_only_its_own_view",
        async |directory| {
            let views = Views::new();
            let (world, bound) = started(directory, &views, BOTH).await;
            let closing = views.clone();
            let tui = world.mode.ui.clone();
            let called = Cell::new(false);
            let options = Rc::new(RefCell::new(OverlayOptions {
                visible: Some(Rc::new(move |_, _| {
                    if !called.replace(true) {
                        closing.complete(0, "first").unwrap();
                        tui.show_overlay(
                            Rc::new(maestro_tui::Text::new("sibling".into(), 0, 0, None)),
                            None,
                        )
                        .unwrap();
                    }
                    true
                })),
                ..OverlayOptions::default()
            }));
            let result = port(&world.mode).custom(
                bound.views[0].clone(),
                Some(overlay(
                    Some(OverlayOptionsSource::Static(options)),
                    Some(bound.on_handle),
                )),
            );
            assert_eq!(result.await.unwrap().as_deref(), Some("first"));
            assert!(world.mode.ui.has_overlay(), "the sibling remains installed");
            world.mode.ui.hide_overlay().unwrap();
            assert!(
                !world.mode.ui.has_overlay(),
                "the completed view was removed by identity"
            );
            assert_eq!(*views.trace.borrow(), ["CUSTOM:dispose"]);
            assert!(views.built.borrow()[0].upgrade().is_none());
        },
    );
}

/// Builds a view whose disposal tries to commit a value after admission fails.
fn admission_failure_factory(views: &Rc<Views>, api: &ExtensionAPI<Local>) -> ViewFactory {
    let (building, api) = (views.clone(), api.clone());
    api.clone().ui_callback(
        move |(_, _, _, done): (TUI, LiveTheme, ExtensionKeybindings, Done), _| {
            let mut component = building.component(&api, "CUSTOM", plain(Flow::Ready));
            let disposing = building.clone();
            component.dispose = Some(api.ui_callback(move |(), _| {
                disposing.mark("CUSTOM:dispose");
                done("ignored".into())
            }));
            Ok(CustomFactoryOutcome::Ready(component))
        },
    )
}

#[test]
fn custom_overlay_admission_cursor_failure_disposes_the_view() {
    isolated(
        "custom_overlay_admission_cursor_failure_disposes_the_view",
        async |directory| {
            let views = Views::new();
            let building = views.clone();
            let (world, mut factories) = with_factories(directory, move |api| {
                vec![admission_failure_factory(&building, api)]
            })
            .await;
            world
                .terminal
                .set_hide_error(Some(std::io::ErrorKind::BrokenPipe));
            let result =
                port(&world.mode).custom(factories.pop().unwrap(), Some(overlay(None, None)));
            let error = result.await.unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::BrokenPipe
            );
            assert!(!world.mode.ui.has_overlay());
            assert_eq!(*views.trace.borrow(), ["CUSTOM:dispose"]);
            assert!(views.built.borrow()[0].upgrade().is_none());
            world.terminal.set_hide_error(None);
        },
    );
}

#[test]
fn custom_overlay_cleanup_keeps_the_original_callback_error() {
    isolated(
        "custom_overlay_cleanup_keeps_the_original_callback_error",
        async |directory| {
            let views = Views::new();
            let terminal = Rc::new(RefCell::new(None::<super::RecordingTerminal>));
            let (building, failing, output) = (views.clone(), views.clone(), terminal.clone());
            let (world, mut bound) = with_factories(directory, move |api| {
                let (failing, output) = (failing.clone(), output.clone());
                let callback = api.ui_callback(move |_: Handle, _| {
                    output
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .set_hide_error(Some(std::io::ErrorKind::BrokenPipe));
                    Err(failing.failure())
                });
                vec![(building.bind(api, BOTH), callback)]
            })
            .await;
            terminal.replace(Some(world.terminal.clone()));
            let (bound, callback) = bound.pop().unwrap();
            let result = port(&world.mode)
                .custom(bound.views[0].clone(), Some(overlay(None, Some(callback))));
            let error = result.await.unwrap_err();
            assert!(views.is_original(error.as_ref()));
            assert!(!world.mode.ui.has_overlay());
            assert_eq!(*views.trace.borrow(), ["CUSTOM:dispose"]);
            world.terminal.set_hide_error(None);
        },
    );
}

/// Whether the editor area shows the view labelled `label`.
fn shows_view(world: &World, label: &str) -> bool {
    world
        .mode
        .editor_container
        .render(80)
        .iter()
        .any(|row| row == label)
}

#[test]
fn inline_custom_views_stack_and_reveal_the_one_below() {
    isolated(
        "inline_custom_views_stack_and_reveal_the_one_below",
        async |directory| {
            let views = Views::new();
            let (world, bound) = started(directory, &views, TWO).await;
            let host = port(&world.mode);
            let first = host.custom(bound.views[0].clone(), None);
            let second = host.custom(bound.views[1].clone(), None);
            world.runtime.settle().unwrap();
            assert!(shows_view(&world, "B") && !shows_view(&world, "A"));
            views.complete(1, "two").unwrap();
            assert_eq!(second.await.unwrap().as_deref(), Some("two"));
            assert!(shows_view(&world, "A"));
            assert!(views.view(0).focus.get());
            views.complete(0, "one").unwrap();
            assert_eq!(first.await.unwrap().as_deref(), Some("one"));
            assert!(restored(&world, "saved"));
        },
    );
}

#[test]
fn dialog_opened_from_a_custom_view_returns_to_it() {
    isolated(
        "dialog_opened_from_a_custom_view_returns_to_it",
        async |directory| {
            let views = Views::new();
            let (world, bound) = started(directory, &views, TWO).await;
            let host = port(&world.mode);
            let view = host.custom(bound.views[0].clone(), None);
            world.runtime.settle().unwrap();
            let mut confirm = host.confirm("Sure".to_owned(), "?".to_owned(), None);
            assert!(!shows_view(&world, "A"));
            world.terminal.send_input("\x1b");
            assert_eq!(
                super::typed::poll(&mut confirm).map(Result::unwrap),
                Some(false)
            );
            assert!(shows_view(&world, "A"));
            assert!(views.view(0).focus.get());
            views.complete(0, "kept").unwrap();
            assert_eq!(view.await.unwrap().as_deref(), Some("kept"));
            assert!(restored(&world, "saved"));
        },
    );
}

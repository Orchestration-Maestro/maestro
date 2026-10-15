//! Retained scope discovery composed from the existing header and list.
use super::{
    SessionList, SessionListOptions, SessionScope, SessionSelectorHeader,
    SessionSelectorHeaderOptions, StatusMessage,
};
use crate::{NameFilter, SortMode, components::dynamic_border::DynamicBorder};
use maestro_app::{ResourceOperations, SessionInfo};
use maestro_theme::{LiveTheme, ThemeColor, paint};
use maestro_tui::{
    Component, Container, FocusFlag, Focusable, KeybindingsManager, Spacer,
    tui::{InputHandler, TuiRuntime},
};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    time::Duration,
};
/// Owned discovery progress notification.
pub type SessionListProgress = Rc<dyn Fn(usize, usize)>;
/// Caller-owned asynchronous discovery result.
pub type SessionsFuture =
    Pin<Box<dyn Future<Output = Result<Vec<SessionInfo>, Box<dyn Error>>> + 'static>>;
/// Session discovery factory, invoked before scheduling its future.
pub type SessionsLoader = Rc<dyn Fn(Option<SessionListProgress>) -> SessionsFuture>;
/// Caller notifications for selection, cancellation and explicit exit.
pub struct SessionSelectorCallbacks {
    /// Selected authored session path.
    pub on_select: Rc<dyn Fn(&str)>,
    /// Cancellation notification.
    pub on_cancel: Rc<dyn Fn()>,
    /// Explicit exit notification.
    pub on_exit: Rc<dyn Fn()>,
}
/// Supplied discovery presentation and host observations.
pub struct SessionSelectorOptions {
    /// Retained live manager for named filtering.
    pub keybindings: KeybindingsManager,
    /// Explicit rename help visibility.
    pub show_rename_hint: bool,
    /// Current authored session path.
    pub current_session_file_path: Option<String>,
    /// Published presentation palette.
    pub theme: LiveTheme,
    /// Existing scheduling owner.
    pub runtime: Rc<dyn TuiRuntime>,
    /// Redraw notification.
    pub request_render: Rc<dyn Fn()>,
    /// Live epoch milliseconds.
    pub now_ms: Rc<dyn Fn() -> f64>,
    /// Live home spelling.
    pub home: Rc<dyn Fn() -> String>,
    /// Canonical path observation port.
    pub paths: Rc<dyn ResourceOperations>,
    /// Live decoration policy.
    pub styles_enabled: Rc<dyn Fn() -> bool>,
}
/// One scope's successful snapshot and pending discovery.
struct Discovery {
    /// Successful result, distinct from unknown or failed discovery.
    records: RefCell<Option<Rc<Vec<SessionInfo>>>>,
    /// Whether the admitted future is unsettled.
    loading: Cell<bool>,
    /// Caller discovery effect.
    loader: SessionsLoader,
}
/// Shared policy retained by list callbacks without a back-reference cycle.
struct State {
    /// Scope currently visible.
    scope: Cell<SessionScope>,
    /// Retained ordering policy.
    sort: Cell<SortMode>,
    /// Retained membership policy.
    name: Cell<NameFilter>,
    /// Retained current and all discovery owners.
    discoveries: [Discovery; 2],
    /// Non-owning publication destination.
    list: Weak<SessionList>,
    /// Loading and status presentation.
    header: Rc<SessionSelectorHeader>,
    /// Scheduling owner.
    runtime: Rc<dyn TuiRuntime>,
    /// Redraw notification.
    render: Rc<dyn Fn()>,
    /// Original cancellation, independent of list slot replacement.
    cancel: Rc<dyn Fn()>,
    /// Whether empty discovery already delivered cancellation.
    cancelled: Cell<bool>,
}
/// Session discovery component with an editable inherited child array.
pub struct SessionSelectorComponent {
    /// Original layout owner.
    container: Container,
    /// Retained searchable list.
    list: Rc<SessionList>,
    /// Independent focus forwarding to the list.
    focus: FocusFlag,
}
impl SessionSelectorComponent {
    /// Starts Current discovery immediately through supplied factories.
    #[must_use]
    pub fn new(
        current_sessions_loader: SessionsLoader,
        all_sessions_loader: SessionsLoader,
        callbacks: SessionSelectorCallbacks,
        options: SessionSelectorOptions,
    ) -> Rc<Self> {
        let header = SessionSelectorHeader::new(
            SessionScope::Current,
            SortMode::Threaded,
            NameFilter::All,
            SessionSelectorHeaderOptions {
                theme: options.theme.clone(),
                runtime: options.runtime.clone(),
                request_render: options.request_render.clone(),
                styles_enabled: options.styles_enabled.clone(),
            },
        );
        header.set_show_rename_hint(options.show_rename_hint);
        let list = SessionList::new(
            Vec::new(),
            SessionListOptions {
                show_cwd: false,
                sort_mode: SortMode::Threaded,
                name_filter: NameFilter::All,
                current_session_file_path: options.current_session_file_path,
                theme: options.theme.clone(),
                now_ms: options.now_ms,
                home: options.home,
                paths: options.paths,
                styles_enabled: options.styles_enabled,
            },
        );
        list.configure_keybindings(options.keybindings);
        let state = Rc::new(State {
            scope: Cell::new(SessionScope::Current),
            sort: Cell::new(SortMode::Threaded),
            name: Cell::new(NameFilter::All),
            discoveries: [current_sessions_loader, all_sessions_loader].map(|loader| Discovery {
                records: RefCell::default(),
                loading: Cell::new(false),
                loader,
            }),
            list: Rc::downgrade(&list),
            header: header.clone(),
            runtime: options.runtime,
            render: options.request_render,
            cancel: callbacks.on_cancel.clone(),
            cancelled: Cell::new(false),
        });
        state.wire(&list, callbacks);
        let container = layout(list.clone(), header, options.theme);
        let component = Rc::new(Self {
            container,
            focus: FocusFlag::forwarding(list.clone()),
            list,
        });
        state.start(SessionScope::Current);
        component
    }
    /// Returns the same retained list, including its mutable callback slots.
    #[must_use]
    pub fn get_session_list(&self) -> Rc<SessionList> {
        self.list.clone()
    }
}
impl State {
    /// Retains each callback independently, with only a weak list edge.
    fn wire(self: &Rc<Self>, list: &SessionList, callbacks: SessionSelectorCallbacks) {
        let selected_header = self.header.clone();
        list.set_on_select(Some(Rc::new(move |path| {
            selected_header.set_status_message(None, None);
            (callbacks.on_select)(path);
        })));
        let cancelled_header = self.header.clone();
        list.set_on_cancel(Some(Rc::new(move || {
            cancelled_header.set_status_message(None, None);
            (callbacks.on_cancel)();
        })));
        let exited_header = self.header.clone();
        list.set_on_exit(Rc::new(move || {
            exited_header.set_status_message(None, None);
            (callbacks.on_exit)();
        }));
        let sorted = self.clone();
        list.set_on_toggle_sort(Some(Rc::new(move || sorted.toggle_sort())));
        let named = self.clone();
        list.set_on_toggle_name_filter(Some(Rc::new(move || named.toggle_name())));
        let path = self.clone();
        list.set_on_toggle_path(Some(Rc::new(move |show| {
            path.header.set_show_path(show);
            (path.render)();
        })));
        let scope_state = self.clone();
        list.set_on_toggle_scope(Some(Rc::new(move || scope_state.toggle_scope())));
    }

    /// Cycles ordering through the existing list setter.
    fn toggle_sort(&self) {
        let mode = match self.sort.get() {
            SortMode::Threaded => SortMode::Recent,
            SortMode::Recent => SortMode::Relevance,
            SortMode::Relevance => SortMode::Threaded,
        };
        self.sort.set(mode);
        self.header.set_sort_mode(mode);
        if let Some(list) = self.list.upgrade() {
            list.set_sort_mode(mode);
        }
        (self.render)();
    }
    /// Toggles membership through the existing list setter.
    fn toggle_name(&self) {
        let filter = if self.name.get() == NameFilter::All {
            NameFilter::Named
        } else {
            NameFilter::All
        };
        self.name.set(filter);
        self.header.set_name_filter(filter);
        if let Some(list) = self.list.upgrade() {
            list.set_name_filter(filter);
        }
        (self.render)();
    }
    /// Selects the retained discovery owner.
    fn discovery(&self, scope: SessionScope) -> &Discovery {
        &self.discoveries[match scope {
            SessionScope::Current => 0,
            SessionScope::All => 1,
        }]
    }
    /// Publishes loading before invoking caller code and scheduling owned work.
    fn start(self: &Rc<Self>, scope: SessionScope) {
        let Some(list) = self.list.upgrade() else {
            return;
        };
        let discovery = self.discovery(scope);
        discovery.loading.set(true);
        self.header.set_scope(scope);
        self.header.set_loading(true);
        (self.render)();
        let weak = Rc::downgrade(self);
        let progress = Rc::new(move |loaded, total| {
            if let Some(state) = weak.upgrade().filter(|state| state.scope.get() == scope) {
                state.header.set_progress(loaded, total);
                (state.render)();
            }
        });
        let future = (discovery.loader)(Some(progress));
        let state = self.clone();
        self.runtime.spawn_local(Box::pin(async move {
            state.complete(scope, &list, future.await);
            Ok(())
        }));
    }
    /// Records settled results before publishing only the active scope.
    fn complete(
        &self,
        scope: SessionScope,
        list: &SessionList,
        result: Result<Vec<SessionInfo>, Box<dyn Error>>,
    ) {
        let discovery = self.discovery(scope);
        discovery.loading.set(false);
        match result {
            Ok(records) => {
                discovery.records.replace(Some(Rc::new(records)));
                if scope == self.scope.get() {
                    self.header.set_loading(false);
                    self.publish(list);
                    (self.render)();
                }
                if self.scope.get() == SessionScope::All
                    && self.discoveries.iter().all(|d| {
                        !d.loading.get()
                            && d.records.borrow().as_ref().is_some_and(|r| r.is_empty())
                    })
                    && !self.cancelled.replace(true)
                {
                    (self.cancel)();
                }
            }
            Err(error) => {
                if scope != self.scope.get() {
                    return;
                }
                self.header.set_loading(false);
                self.header.set_status_message(
                    Some(StatusMessage::Error(format!(
                        "Failed to load sessions: {error}"
                    ))),
                    Some(Duration::from_millis(4000)),
                );
                if scope == SessionScope::Current {
                    list.install_sessions(Rc::default(), false);
                }
                (self.render)();
            }
        }
    }
    /// Installs a shared successful snapshot or an empty fallback.
    fn publish(&self, list: &SessionList) {
        let records = self
            .discovery(self.scope.get())
            .records
            .borrow()
            .clone()
            .unwrap_or_default();
        list.install_sessions(records, self.scope.get() == SessionScope::All);
    }
    /// Selects caches or reuses an admitted discovery without restarting it.
    fn toggle_scope(self: &Rc<Self>) {
        let scope = match self.scope.get() {
            SessionScope::Current => SessionScope::All,
            SessionScope::All => SessionScope::Current,
        };
        self.scope.set(scope);
        self.header.set_scope(scope);
        let discovery = self.discovery(scope);
        if scope == SessionScope::All
            && discovery.records.borrow().is_none()
            && !discovery.loading.get()
        {
            self.start(scope);
            return;
        }
        self.header.set_loading(discovery.loading.get());
        if (scope == SessionScope::Current || discovery.records.borrow().is_some())
            && let Some(list) = self.list.upgrade()
        {
            self.publish(&list);
        }
        (self.render)();
    }
}
/// Composes original children through the toolkit's one container owner.
fn layout(list: Rc<SessionList>, header: Rc<SessionSelectorHeader>, theme: LiveTheme) -> Container {
    let container = Container::new();
    container.add_child(Rc::new(Spacer::new(1)));
    let border_theme = theme.clone();
    let border = Rc::new(move |text: &str| {
        paint(
            border_theme.get().ok().as_deref(),
            &ThemeColor::Accent,
            text,
        )
    });
    container.add_child(Rc::new(DynamicBorder::new(
        theme.clone(),
        Some(border.clone()),
    )));
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(header);
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(list);
    container.add_child(Rc::new(Spacer::new(1)));
    container.add_child(Rc::new(DynamicBorder::new(theme, Some(border))));
    container
}
impl Component for SessionSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn Error>> {
        self.container.try_render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
impl InputHandler for SessionSelectorComponent {
    fn handle_input(&self, data: &str) {
        self.list.handle_input(data);
    }
}
impl Focusable for SessionSelectorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
component_children!(SessionSelectorComponent, Container, container);

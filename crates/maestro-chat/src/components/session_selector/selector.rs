//! Retained scope discovery composed from the existing header and list.
use super::delete_session_file::SessionFileOperations;
use super::{
    SessionList, SessionListOptions, SessionScope, SessionSelectorHeader,
    SessionSelectorHeaderOptions, StatusMessage,
};
use crate::{NameFilter, SortMode, components::dynamic_border::DynamicBorder};
use maestro_app::SessionInfo;
use maestro_theme::{LiveTheme, ThemeColor, paint};
use maestro_tui::{
    Component, Container, FocusFlag, Focusable, KeybindingsManager, Spacer,
    tui::{InputHandler, TuiRuntime},
};
use maestro_tui::{Input, tui::LocalFuture};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::{Rc, Weak},
    time::Duration,
};
#[path = "mutations.rs"]
mod mutations;
#[path = "rename.rs"]
mod rename;
/// Owned asynchronous name-save callable.
type RenameSession = Rc<dyn Fn(&str, &str) -> LocalFuture>;
pub use maestro_app::presentation_data::input_selectors::{
    SessionListProgress, SessionSelectorCallbacks, SessionsFuture, SessionsLoader,
};
/// Supplied discovery presentation and host observations.
pub struct SessionSelectorOptions {
    /// Retained live manager for named filtering; omitted values load application defaults.
    pub keybindings: Option<KeybindingsManager>,
    /// Explicit rename help visibility.
    pub show_rename_hint: Option<bool>,
    /// Saves a new session name through caller-owned work.
    pub rename_session: Option<RenameSession>,
    /// Supplied deletion effects, or native effects when omitted.
    pub file_operations: Option<Rc<dyn SessionFileOperations>>,
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
    /// Live decoration policy.
    pub styles_enabled: Rc<dyn Fn() -> bool>,
}
/// One scope's successful snapshot and pending discovery.
struct Discovery {
    /// Successful result, distinct from unknown or failed discovery.
    records: RefCell<Option<Rc<Vec<Rc<SessionInfo>>>>>,
    /// Whether the admitted future is unsettled.
    loading: Cell<bool>,
    /// Caller discovery effect.
    loader: SessionsLoader,
    /// Identity of the latest admitted request.
    token: RefCell<Rc<()>>,
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
    /// Non-owning original layout destination.
    container: Weak<Container>,
    /// Stable rename editor shared across openings.
    rename_input: Rc<Input>,
    /// Authored target retained while rename is visible.
    rename_target: RefCell<Option<String>>,
    /// Caller-owned save operation.
    rename: Option<RenameSession>,
    /// Prepared text palette and live border owner.
    theme: LiveTheme,
    /// Live decoration policy for prepared text.
    styles: Rc<dyn Fn() -> bool>,
    /// Retained deletion effects.
    file_operations: Option<Rc<dyn SessionFileOperations>>,
}
/// Session discovery component with an editable inherited child array.
pub struct SessionSelectorComponent {
    /// Original layout owner.
    container: Rc<Container>,
    /// Shared mode and discovery owner.
    state: Rc<State>,
    /// Retained searchable list.
    list: Rc<SessionList>,
    /// Independent focus forwarding to the list.
    focus: FocusFlag,
}
impl SessionSelectorComponent {
    /// Resolves keybindings before starting Current discovery through supplied factories.
    ///
    /// # Errors
    /// Returns a configuration-root lookup error when an omitted manager cannot be loaded.
    pub fn new(
        current_sessions_loader: SessionsLoader,
        all_sessions_loader: SessionsLoader,
        callbacks: SessionSelectorCallbacks,
        mut options: SessionSelectorOptions,
    ) -> std::io::Result<Rc<Self>> {
        let keybindings = options.keybindings.take().map_or_else(
            || {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    default_keybindings()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    Ok::<_, std::io::Error>(default_keybindings())
                }
            },
            Ok,
        )?;
        let container = Rc::new(Container::new());
        let list = create_list(&mut options, keybindings);
        let state = State::new(
            options,
            [current_sessions_loader, all_sessions_loader],
            &list,
            &container,
            callbacks.on_cancel.clone(),
        );
        state.wire(&list, callbacks);
        state.wire_rename(&list);
        state.wire_mutations(&list);
        state.build_layout(list.clone(), true);
        let component = Rc::new(Self {
            container,
            focus: FocusFlag::forwarding_all(vec![list.clone(), state.rename_input.clone()]),
            state: state.clone(),
            list,
        });
        state.start(SessionScope::Current, LoadReason::Initial);
        Ok(component)
    }
    /// Returns the same retained list, including its mutable callback slots.
    #[must_use]
    pub fn get_session_list(&self) -> Rc<SessionList> {
        self.list.clone()
    }
}
impl State {
    /// Retains discovery, presentation and mutation owners with weak layout edges.
    fn new(
        options: SessionSelectorOptions,
        loaders: [SessionsLoader; 2],
        list: &Rc<SessionList>,
        container: &Rc<Container>,
        cancel: Rc<dyn Fn()>,
    ) -> Rc<Self> {
        let header = create_header(&options);
        let rename_input = Rc::new(Input::new());
        Rc::new(Self {
            scope: Cell::new(SessionScope::Current),
            sort: Cell::new(SortMode::Threaded),
            name: Cell::new(NameFilter::All),
            discoveries: loaders.map(|loader| Discovery {
                records: RefCell::default(),
                loading: Cell::new(false),
                loader,
                token: RefCell::new(Rc::new(())),
            }),
            list: Rc::downgrade(list),
            header,
            runtime: options.runtime,
            render: options.request_render,
            cancel,
            cancelled: Cell::new(false),
            container: Rc::downgrade(container),
            rename_input,
            rename_target: RefCell::default(),
            rename: options.rename_session,
            theme: options.theme,
            styles: options.styles_enabled,
            file_operations: super::delete_session_file::operations(options.file_operations),
        })
    }

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
    fn start(self: &Rc<Self>, scope: SessionScope, reason: LoadReason) {
        let future = self.load(scope, reason);
        self.runtime.spawn_local(future);
    }
    /// Admits discovery synchronously, returning its owned completion.
    fn load(self: &Rc<Self>, scope: SessionScope, reason: LoadReason) -> LocalFuture {
        let Some(list) = self.list.upgrade() else {
            return Box::pin(async { Ok(()) });
        };
        let discovery = self.discovery(scope);
        let token = Rc::new(());
        discovery.token.replace(token.clone());
        discovery.loading.set(true);
        self.header.set_scope(scope);
        self.header.set_loading(true);
        (self.render)();
        let weak = Rc::downgrade(self);
        let progress_token = token.clone();
        let progress = Rc::new(move |loaded, total| {
            if let Some(state) = weak.upgrade().filter(|state| {
                state.scope.get() == scope
                    && Rc::ptr_eq(&state.discovery(scope).token.borrow(), &progress_token)
            }) {
                state.header.set_progress(loaded, total);
                (state.render)();
            }
        });
        let future = (discovery.loader)(Some(progress));
        let state = self.clone();
        let request = LoadRequest {
            scope,
            reason,
            token,
            list,
        };
        Box::pin(async move {
            state.complete(request, future.await);
            Ok(())
        })
    }
    /// Records settled results before publishing only the active scope.
    fn complete(&self, request: LoadRequest, result: Result<Vec<SessionInfo>, Box<dyn Error>>) {
        let LoadRequest {
            scope,
            reason,
            token,
            list,
        } = request;
        let discovery = self.discovery(scope);
        if !Rc::ptr_eq(&discovery.token.borrow(), &token) {
            return;
        }
        discovery.loading.set(false);
        match result {
            Ok(records) => {
                discovery
                    .records
                    .replace(Some(Rc::new(records.into_iter().map(Rc::new).collect())));
                if scope == self.scope.get() {
                    self.header.set_loading(false);
                    self.publish(&list);
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
                if matches!(reason, LoadReason::Initial) {
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
            self.start(scope, LoadReason::Toggle);
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
        if self.state.rename_target.borrow().is_some() {
            if maestro_tui::get_keybindings().matches(data, "tui.select.cancel") {
                self.state.exit_rename();
            } else {
                self.state.rename_input.handle_input(data);
            }
        } else {
            self.list.handle_input(data);
        }
    }
}
impl Focusable for SessionSelectorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
component_children!(SessionSelectorComponent, Container, container);

/// Constructs the retained header from the caller's presentation observations.
fn create_header(options: &SessionSelectorOptions) -> Rc<SessionSelectorHeader> {
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
    header.set_show_rename_hint(
        options
            .show_rename_hint
            .unwrap_or(options.rename_session.is_some()),
    );
    header
}
/// Load the shared native manager.
#[cfg(not(target_arch = "wasm32"))]
fn default_keybindings() -> std::io::Result<KeybindingsManager> {
    let manager = maestro_app::KeybindingsManager::create(None)?;
    Ok(KeybindingsManager::clone(&manager))
}
/// Retain in-memory defaults on browser targets.
#[cfg(target_arch = "wasm32")]
fn default_keybindings() -> KeybindingsManager {
    KeybindingsManager::clone(&maestro_app::KeybindingsManager::default())
}

/// Why an admitted discovery is running.
#[derive(Clone, Copy)]
enum LoadReason {
    /// First constructor population.
    Initial,
    /// Mutation reconciliation.
    Refresh,
    /// Uncached scope selection.
    Toggle,
}
/// Owned request identity and publication destination.
struct LoadRequest {
    /// Discovery scope.
    scope: SessionScope,
    /// Failure publication policy.
    reason: LoadReason,
    /// Admission identity.
    token: Rc<()>,
    /// Retained publication destination.
    list: Rc<SessionList>,
}

/// Builds the original list before mutation callbacks are installed.
fn create_list(
    options: &mut SessionSelectorOptions,
    keybindings: KeybindingsManager,
) -> Rc<SessionList> {
    let list = SessionList::new(
        Vec::new(),
        SessionListOptions {
            runtime: options.runtime.clone(),
            show_cwd: false,
            sort_mode: SortMode::Threaded,
            name_filter: NameFilter::All,
            current_session_file_path: options.current_session_file_path.take(),
            theme: options.theme.clone(),
            now_ms: options.now_ms.clone(),
            home: options.home.clone(),
            styles_enabled: options.styles_enabled.clone(),
        },
    );
    list.configure_keybindings(keybindings);
    list
}

//! Provider authentication choices with retained search and selection state.
use super::dynamic_border::DynamicBorder;
use maestro_app::{AuthCredential, AuthSource, AuthStatus, AuthStorage};
use maestro_theme::{LiveTheme, Theme, ThemeColor, paint};
use maestro_tui::{
    Component, Container, FocusFlag, Focusable, Input, Spacer, TruncatedText,
    tui::{InputHandler, TuiRuntime},
};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
    sync::Arc,
};

/// The authentication kind offered for a provider.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    /// Subscription authentication.
    OAuth,
    /// API-key authentication.
    ApiKey,
}
/// A caller-supplied provider choice.
#[derive(Clone)]
pub struct AuthSelectorProvider {
    /// Open provider identity reported on selection.
    pub id: String,
    /// Displayed provider name.
    pub name: String,
    /// Offered authentication kind.
    pub auth_type: AuthType,
}
/// The operation whose provider is being selected.
#[derive(Clone, Copy)]
pub enum OAuthSelectorMode {
    /// Configure authentication.
    Login,
    /// Remove authentication through the caller.
    Logout,
}
/// Caller-supplied authentication metadata reader.
type AuthStatusResolver = Rc<dyn Fn(&str) -> Result<AuthStatus, Box<dyn Error>>>;

/// Constructor inputs for presentation and authentication metadata.
pub struct OAuthSelectorOptions {
    /// Operation title and empty-state wording.
    pub mode: OAuthSelectorMode,
    /// Published theme used by the picker.
    pub theme: LiveTheme,
    /// Optional fallible metadata resolver replacing the credential store's default.
    pub get_auth_status: Option<AuthStatusResolver>,
    /// Host receiving failures from legacy infallible input dispatch.
    pub runtime: Rc<dyn TuiRuntime>,
}
/// Selection state shared with a retained search-input callback.
struct Selection {
    /// Immutable ordered choices, including duplicates.
    providers: Vec<AuthSelectorProvider>,
    /// Indexes of choices in the current filtered order.
    filtered: RefCell<Vec<usize>>,
    /// Selected position within that order.
    index: Cell<usize>,
    /// Caller notification, invoked without borrowed state.
    select: Rc<dyn Fn(&str)>,
}
impl Selection {
    /// Reads a current choice without holding a mutable-state borrow.
    fn provider(&self, index: usize) -> Option<&AuthSelectorProvider> {
        let source = self.filtered.borrow().get(index).copied()?;
        self.providers.get(source)
    }
    /// Reports the currently selected identity, when the view is nonempty.
    fn submit(&self) {
        if let Some(provider) = self.provider(self.index.get()) {
            (self.select)(&provider.id);
        }
    }
}
/// A provider picker; authentication work belongs to its caller.
pub struct OAuthSelectorComponent {
    /// Editable outer child-array owner.
    container: Container,
    /// Retained search child, even when removed from the outer array.
    input: Rc<Input>,
    /// Retained list owner.
    list: Rc<Container>,
    /// Shared selection without a back-reference to this component.
    selection: Rc<Selection>,
    /// Shared credential reader.
    storage: Arc<AuthStorage>,
    /// Caller presentation and metadata choices.
    options: OAuthSelectorOptions,
    /// Cancellation notification.
    cancel: Rc<dyn Fn()>,
    /// Parent focus, forwarded immediately to the input.
    focus: FocusFlag,
}
impl OAuthSelectorComponent {
    /// Creates the picker and immediately builds its initial visible rows.
    ///
    /// # Errors
    /// Returns the original metadata failure from initial row construction.
    pub fn new(
        storage: Arc<AuthStorage>,
        providers: Vec<AuthSelectorProvider>,
        on_select: Rc<dyn Fn(&str)>,
        on_cancel: Rc<dyn Fn()>,
        options: OAuthSelectorOptions,
    ) -> Result<Self, Box<dyn Error>> {
        let selection = Rc::new(Selection {
            filtered: RefCell::new((0..providers.len()).collect()),
            providers,
            index: Cell::new(0),
            select: on_select,
        });
        let input = Rc::new(Input::new());
        let submitted = selection.clone();
        input.set_on_submit(Some(Rc::new(move |_| submitted.submit())));
        let container = Container::new();
        let title = match options.mode {
            OAuthSelectorMode::Login => "Select provider to configure:",
            OAuthSelectorMode::Logout => "Select provider to logout:",
        };
        container.add_child(Rc::new(DynamicBorder::new(options.theme.clone(), None)));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(TruncatedText::new(
            paint(
                options.theme.get().ok().as_deref(),
                &ThemeColor::Accent,
                &Theme::bold(title, true),
            ),
            1,
            0,
        )));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(input.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        let list = Rc::new(Container::new());
        container.add_child(list.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(DynamicBorder::new(options.theme.clone(), None)));
        let panel = Self {
            container,
            focus: FocusFlag::forwarding(input.clone()),
            input,
            list,
            selection,
            storage,
            options,
            cancel: on_cancel,
        };
        panel.rebuild()?;
        Ok(panel)
    }
    /// Filters through the shared fuzzy owner and clamps the existing position.
    fn filter(&self) -> Result<(), Box<dyn Error>> {
        let all: Vec<_> = (0..self.selection.providers.len()).collect();
        let filtered = maestro_tui::fuzzy_filter(&all, &self.input.get_value(), |index| {
            let provider = &self.selection.providers[*index];
            format!(
                "{} {} {}",
                provider.name,
                provider.id,
                match provider.auth_type {
                    AuthType::OAuth => "oauth",
                    AuthType::ApiKey => "api_key",
                }
            )
        })
        .into_iter()
        .copied()
        .collect::<Vec<_>>();
        self.selection.index.set(
            self.selection
                .index
                .get()
                .min(filtered.len().saturating_sub(1)),
        );
        self.selection.filtered.replace(filtered);
        self.rebuild()
    }
    /// Colors one captured list string with the current role.
    fn paint(&self, role: &ThemeColor, text: &str) -> String {
        paint(self.options.theme.get().ok().as_deref(), role, text)
    }
    /// Adds one padded, toolkit-truncated row.
    fn row(&self, text: String) {
        self.list.add_child(Rc::new(TruncatedText::new(text, 1, 0)));
    }
    /// Formats decoded stored authentication without resolving key values.
    fn status(&self, provider: &AuthSelectorProvider) -> Result<String, Box<dyn Error>> {
        if let Some(credential) = self.storage.get(&provider.id) {
            let oauth = matches!(credential, AuthCredential::OAuth(_));
            if oauth == (provider.auth_type == AuthType::OAuth) {
                return Ok(self.paint(&ThemeColor::Success, " ✓ configured"));
            }
            return Ok(self.paint(&ThemeColor::Muted, " • ")
                + &self.paint(
                    &ThemeColor::Warning,
                    if oauth {
                        "subscription configured"
                    } else {
                        "API key configured"
                    },
                ));
        }
        if provider.auth_type == AuthType::OAuth {
            return Ok(self.paint(&ThemeColor::Muted, " • unconfigured"));
        }
        let status = self.options.get_auth_status.as_ref().map_or_else(
            || {
                self.storage
                    .get_auth_status(&provider.id)
                    .map_err(Into::into)
            },
            |resolve| resolve(&provider.id),
        )?;
        let label = match status.source {
            Some(AuthSource::Environment) => {
                format!(" ✓ env: {}", status.label.as_deref().unwrap_or("API key"))
            }
            Some(AuthSource::Runtime) => " ✓ runtime API key".into(),
            Some(AuthSource::Fallback) => " ✓ custom API key".into(),
            Some(AuthSource::ModelsJsonKey) => " ✓ key in models.json".into(),
            Some(AuthSource::ModelsJsonCommand) => " ✓ command in models.json".into(),
            _ => return Ok(self.paint(&ThemeColor::Muted, " • unconfigured")),
        };
        Ok(self.paint(&ThemeColor::Success, &label))
    }
    /// Rebuilds the bounded list without refreshing on ordinary rendering.
    fn rebuild(&self) -> Result<(), Box<dyn Error>> {
        self.list.clear();
        let count = self.selection.filtered.borrow().len();
        let selected = self.selection.index.get();
        let window = super::list_window(selected, count, 8);
        let (start, end) = (window.start, window.end);
        for index in start..end {
            let Some(provider) = self.selection.provider(index) else {
                continue;
            };
            let selected = index == self.selection.index.get();
            let status = self.status(provider)?;
            let name = if selected {
                self.paint(&ThemeColor::Accent, "→ ")
                    + &self.paint(&ThemeColor::Accent, &provider.name)
            } else {
                "  ".to_owned() + &self.paint(&ThemeColor::Text, &provider.name)
            };
            self.row(name + &status);
        }
        let count = self.selection.filtered.borrow().len();
        if start > 0 || end < count {
            self.row(self.paint(
                &ThemeColor::Muted,
                &format!("  ({}/{count})", self.selection.index.get() + 1),
            ));
        }
        if count == 0 {
            let message = if self.selection.providers.is_empty() {
                match self.options.mode {
                    OAuthSelectorMode::Login => "No providers available",
                    OAuthSelectorMode::Logout => "No providers logged in. Use /login first.",
                }
            } else {
                "No matching providers"
            };
            self.row(self.paint(&ThemeColor::Muted, &format!("  {message}")));
        }
        Ok(())
    }
    /// Handles one input, retaining effects completed before a metadata failure.
    ///
    /// # Errors
    /// Returns the original row metadata failure without reporting it to the host.
    pub fn handle_input(&self, data: &str) -> Result<(), Box<dyn Error>> {
        let bindings = maestro_tui::get_keybindings();
        let count = self.selection.filtered.borrow().len();
        if bindings.matches(data, "tui.select.up") {
            if count == 0 {
                return Ok(());
            }
            self.selection
                .index
                .set(self.selection.index.get().saturating_sub(1));
            self.rebuild()?;
        } else if bindings.matches(data, "tui.select.down") {
            if count == 0 {
                return Ok(());
            }
            self.selection
                .index
                .set((self.selection.index.get() + 1).min(count - 1));
            self.rebuild()?;
        } else if bindings.matches(data, "tui.select.confirm") {
            self.selection.submit();
        } else if bindings.matches(data, "tui.select.cancel") {
            (self.cancel)();
        } else {
            self.input.handle_input(data);
            self.filter()?;
        }
        Ok(())
    }
}
impl InputHandler for OAuthSelectorComponent {
    fn handle_input(&self, data: &str) {
        if let Err(error) = OAuthSelectorComponent::handle_input(self, data) {
            self.options
                .runtime
                .spawn_local(Box::pin(std::future::ready(Err(error))));
        }
    }
    fn try_handle_input(&self, data: &str) -> Result<(), Box<dyn Error>> {
        OAuthSelectorComponent::handle_input(self, data)
    }
}
impl Component for OAuthSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
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
impl Focusable for OAuthSelectorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
component_children!(OAuthSelectorComponent, Container, container);

//! The native appearance button, its stored choice and its platform observation.
use super::{THEME_KEY, initial_theme, is_dark, next_theme, theme_icon};
use crate::{
    browser::{attachment_content::browser::host_error, event_subscription::EventSubscription},
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{DomView, report},
    icons::IconSize,
    tools::{renderer_registry::header::dom::glyph, types::ToolUiError},
};
use maestro_theme::BrandMode;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, Event, HtmlButtonElement, HtmlElement, MediaQueryList, Storage};

/// One retained native listener.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// The platform's dark preference query.
const DARK_QUERY: &str = "(prefers-color-scheme: dark)";

/// A button cycling the page between light, dark and, optionally, the platform's appearance.
///
/// The choice is stored in the page's local storage; system removes it. The page shows the
/// selected pack's dark or light colors, and in system mode follows platform changes while the
/// button exists.
pub struct ThemeToggle {
    /// Shared state the listeners reach weakly.
    state: Rc<Toggle>,
}
/// The button and its appearance state.
struct Toggle {
    /// Page operands.
    view: Rc<DomView>,
    /// Whether the cycle includes system.
    include_system: bool,
    /// The appearance shown.
    theme: RefCell<String>,
    /// The ghost icon button.
    button: HtmlButtonElement,
    /// The platform's dark preference, when the platform reports one.
    media: Option<MediaQueryList>,
    /// Activation and platform listeners.
    listeners: RefCell<Vec<Listener>>,
}
impl ThemeToggle {
    /// Build the button, show the stored appearance and apply it to the page.
    /// # Errors
    /// Returns an icon, brand or native failure.
    pub fn new(view: &Rc<DomView>, include_system: bool) -> Result<Self, ToolUiError> {
        crate::dom::install_styles(view).map_err(|error| host_error(&error))?;
        let button = button(
            &view.document,
            &ButtonProps {
                variant: ButtonVariant::Ghost,
                size: ButtonSize::Icon,
                class_name: "maestro-theme-toggle",
                title: Some(&view.i18n.text("Toggle theme")?),
                ..ButtonProps::default()
            },
            &[],
        )
        .map_err(|error| host_error(&error))?;
        let media = view
            .document
            .default_view()
            .and_then(|window| window.match_media(DARK_QUERY).ok().flatten());
        let state = Rc::new(Toggle {
            view: Rc::clone(view),
            include_system,
            theme: RefCell::new(initial_theme(stored(view).as_deref()).to_owned()),
            button,
            media,
            listeners: RefCell::default(),
        });
        state.bind()?;
        state.show()?;
        state.apply()?;
        Ok(Self { state })
    }
    /// The button the caller places.
    #[must_use]
    pub fn root(&self) -> &Element {
        &self.state.button
    }
    /// The appearance shown.
    #[must_use]
    pub fn theme(&self) -> String {
        self.state.theme.borrow().clone()
    }
}
impl Toggle {
    /// Cycle on activation and follow platform changes in system mode.
    fn bind(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let mut listeners = vec![listen(
            self.button.clone().into(),
            "click",
            Rc::downgrade(self),
            |toggle| report(toggle.cycle()),
        )?];
        if let Some(media) = &self.media {
            listeners.push(listen(
                media.clone().into(),
                "change",
                Rc::downgrade(self),
                Toggle::platform_changed,
            )?);
        }
        self.listeners.replace(listeners);
        Ok(())
    }
    /// Follow a platform change unless an explicit light or dark choice is stored.
    fn platform_changed(&self) {
        let stored = stored(&self.view);
        if stored
            .as_deref()
            .is_none_or(|theme| theme.is_empty() || theme == "system")
        {
            report(self.apply());
        }
    }
    /// Store and show the next appearance.
    fn cycle(&self) -> Result<(), ToolUiError> {
        let next = next_theme(&self.theme.borrow(), self.include_system);
        self.theme.replace(next.to_owned());
        if let Some(storage) = local_storage(&self.view) {
            let _ = if next == "system" {
                storage.remove_item(THEME_KEY)
            } else {
                storage.set_item(THEME_KEY, next)
            };
        }
        self.show()?;
        self.apply()
    }
    /// Show the icon of the current appearance.
    fn show(&self) -> Result<(), ToolUiError> {
        let node = glyph(theme_icon(&self.theme.borrow()))?;
        let svg = crate::icons::icon_dom(&node, IconSize::Md, None).map_err(|e| host_error(&e))?;
        self.button.replace_children_with_node_1(&svg);
        Ok(())
    }
    /// Give the page the selected pack's colors for the stored appearance.
    fn apply(&self) -> Result<(), ToolUiError> {
        let theme = stored(&self.view);
        let system_dark = self.media.as_ref().is_some_and(MediaQueryList::matches);
        let mode = if is_dark(initial_theme(theme.as_deref()), system_dark) {
            BrandMode::Dark
        } else {
            BrandMode::Light
        };
        let presentation = self
            .view
            .pack
            .presentation(mode)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))?;
        let Some(root) = self.view.document.document_element() else {
            return Ok(());
        };
        let style = root.unchecked_into::<HtmlElement>().style();
        for (key, value) in presentation.css_properties() {
            style
                .set_property(&key, &value)
                .map_err(|error| host_error(&error))?;
        }
        Ok(())
    }
}
/// One listener on `target` reaching the toggle while it exists.
fn listen(
    target: web_sys::EventTarget,
    name: &'static str,
    toggle: Weak<Toggle>,
    action: impl Fn(&Toggle) + 'static,
) -> Result<Listener, ToolUiError> {
    let callback = Closure::<dyn Fn(Event)>::new(move |_: Event| {
        if let Some(toggle) = toggle.upgrade() {
            action(&toggle);
        }
    });
    EventSubscription::new(target, name, callback).map_err(|error| host_error(&error).into())
}
/// The page's local storage, when available.
fn local_storage(view: &DomView) -> Option<Storage> {
    view.document
        .default_view()
        .and_then(|window| window.local_storage().ok().flatten())
}
/// The stored appearance, if any.
fn stored(view: &DomView) -> Option<String> {
    local_storage(view).and_then(|storage| storage.get_item(THEME_KEY).ok().flatten())
}

//! Loader composition with captured decoration and a construction-time hint.
use super::dynamic_border::DynamicBorder;
use maestro_theme::{LiveTheme, Theme, ThemeColor, ThemeError, paint};
use maestro_tui::editor_component::BorderColor;
use maestro_tui::interface::InputHandler;
use maestro_tui::{CancellableLoader, Cancellation};
use maestro_tui::{Component, Container, Loader, Spacer, TUI, Text};
use std::{ops::Deref, rc::Rc};

/// Selects the cancellable or plain loader.
#[derive(Clone, Copy)]
pub struct BorderedLoaderOptions {
    /// Whether input can cancel this loader.
    pub cancellable: bool,
}
impl Default for BorderedLoaderOptions {
    fn default() -> Self {
        Self { cancellable: true }
    }
}
/// A loader inside an editable container of borders and optional hint.
pub struct BorderedLoader {
    /// Shared child collection owner.
    container: Container,
    /// Retained loader, independent of child-array replacements.
    loader: Inner,
}
/// Distinct cancellation and plain lifecycle owners.
enum Inner {
    /// Input-cancellable owner.
    Cancellable(Rc<CancellableLoader>),
    /// Plain animation with its own signal.
    Plain(Loader, Cancellation),
}
impl BorderedLoader {
    /// Captures decoration from `theme` and, when cancellable, builds the hint from `hint_theme`.
    ///
    /// # Errors
    /// Returns the hint owner's theme failure in cancellable mode.
    pub fn new(
        tui: &TUI,
        theme: Rc<Theme>,
        message: String,
        options: BorderedLoaderOptions,
        hint_theme: &LiveTheme,
    ) -> Result<Self, ThemeError> {
        let color = move |key| {
            let theme = Rc::clone(&theme);
            Rc::new(move |text: &str| paint(Some(&theme), &key, text)) as BorderColor
        };
        let border = color(ThemeColor::Border);
        let loader = Loader::new(
            tui.clone(),
            color(ThemeColor::Accent),
            color(ThemeColor::Muted),
            Some(message),
            None,
        );
        let container = Container::new();
        let decoration = || Rc::new(DynamicBorder::new(hint_theme.clone(), Some(border.clone())));
        container.add_child(decoration());
        let loader = if options.cancellable {
            let loader = Rc::new(CancellableLoader::new(loader));
            container.add_child(loader.clone());
            container.add_child(Rc::new(Spacer::new(1)));
            let hint = maestro_app::key_hint("tui.select.cancel", "cancel", hint_theme)?;
            container.add_child(Rc::new(Text::new(hint, 1, 0, None)));
            Inner::Cancellable(loader)
        } else {
            container.add_child(Rc::new(loader.clone()));
            Inner::Plain(loader, Cancellation::new())
        };
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(decoration());
        Ok(Self { container, loader })
    }
    /// Borrows the retained inner signal.
    #[must_use]
    pub fn signal(&self) -> &Cancellation {
        match &self.loader {
            Inner::Cancellable(loader) => loader.signal(),
            Inner::Plain(_, signal) => signal,
        }
    }
    /// Replaces cancellation notification; the plain form ignores it.
    pub fn set_on_abort(&self, callback: Option<Rc<dyn Fn()>>) {
        if let Inner::Cancellable(loader) = &self.loader {
            loader.set_on_abort(callback);
        }
    }
    /// Stops animation without aborting the signal.
    pub fn dispose(&self) {
        match &self.loader {
            Inner::Cancellable(loader) => loader.dispose(),
            Inner::Plain(loader, _) => loader.stop(),
        }
    }
}
impl Deref for BorderedLoader {
    type Target = Container;
    fn deref(&self) -> &Container {
        &self.container
    }
}
impl InputHandler for BorderedLoader {
    fn handle_input(&self, data: &str) {
        if let Inner::Cancellable(loader) = &self.loader {
            loader.handle_input(data);
        }
    }
}
impl Component for BorderedLoader {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}

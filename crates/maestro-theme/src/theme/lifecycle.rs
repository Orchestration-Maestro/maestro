//! The published theme, its selected name and the single change callback.
use super::fs_watch::ThemeWatchOperations;
use super::watching::Watch;
use super::{Theme, ThemeError, ThemeState};
use std::borrow::Cow;
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

/// Name selected while a directly supplied instance is published.
pub(super) const IN_MEMORY: &str = "<in-memory>";

/// A change callback; a switch reports its failure and a file reload suppresses it.
type Callback = Rc<dyn Fn() -> Result<(), Box<dyn Error + Send + Sync>>>;

/// Mutable lifecycle slots of one [`ThemeState`].
#[derive(Default)]
pub(super) struct Lifecycle {
    /// Shared slot read by every [`LiveTheme`].
    live: Rc<RefCell<Option<Rc<Theme>>>>,
    /// Selected theme name.
    pub(super) name: RefCell<Option<String>>,
    /// The one registered change callback.
    callback: RefCell<Option<Callback>>,
    /// Active directory watch and pending reload.
    pub(super) watch: RefCell<Watch>,
}

/// Handle that reads the currently published theme on every call.
///
/// Cloning clones the handle; every clone reads the same slot.
#[derive(Clone)]
pub struct LiveTheme(Rc<RefCell<Option<Rc<Theme>>>>);

impl LiveTheme {
    /// Return the currently published theme.
    ///
    /// # Errors
    /// Returns an error before any theme was published.
    pub fn get(&self) -> Result<Rc<Theme>, ThemeError> {
        self.0.borrow().clone().ok_or_else(|| {
            ThemeError::message("Theme not initialized. Call initTheme() first.".to_owned())
        })
    }
}

/// Outcome of a named selection that was handled by falling back to `dark`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeChangeResult {
    /// The theme was published and the callback succeeded.
    Success,
    /// Loading or the callback failed; `dark` was published instead.
    Failure {
        /// Message of the original failure.
        error: String,
    },
}

impl ThemeState {
    /// Return a live view of the published theme.
    #[must_use]
    pub fn theme(&self) -> LiveTheme {
        LiveTheme(Rc::clone(&self.lifecycle.live))
    }

    /// Select and publish `name`, or the terminal background's theme when omitted.
    ///
    /// Failure to load `name` publishes `dark` silently; the callback is never invoked.
    /// `None` leaves an existing watch untouched.
    ///
    /// # Errors
    /// Returns the failure to load `dark` after a failed selection.
    pub fn init_theme(
        self: &Rc<Self>,
        name: Option<&str>,
        watcher: Option<Rc<dyn ThemeWatchOperations>>,
    ) -> Result<(), ThemeError> {
        let name = name.map_or_else(|| Cow::Owned(self.default_theme()), Cow::Borrowed);
        self.select(&name);
        let loaded = self.load_named(&name).map(|theme| {
            self.publish(theme);
            self.start_theme_watcher(watcher);
        });
        loaded.or_else(|_| self.fall_back())
    }

    /// Select and publish `name`, then invoke the callback.
    ///
    /// A loading or callback failure publishes `dark` and is reported in the result.
    /// `None` leaves an existing watch untouched.
    ///
    /// # Errors
    /// Returns the failure to load `dark` after a failed selection.
    pub fn set_theme(
        self: &Rc<Self>,
        name: &str,
        watcher: Option<Rc<dyn ThemeWatchOperations>>,
    ) -> Result<ThemeChangeResult, ThemeError> {
        self.select(name);
        let switched = self.load_named(name).and_then(|theme| {
            self.publish(theme);
            self.start_theme_watcher(watcher);
            self.notify()
        });
        match switched {
            Ok(()) => Ok(ThemeChangeResult::Success),
            Err(error) => {
                self.fall_back()?;
                Ok(ThemeChangeResult::Failure {
                    error: error.to_string(),
                })
            }
        }
    }

    /// Publish a directly supplied instance, stop watching and invoke the callback.
    ///
    /// # Errors
    /// Returns the callback's failure; the operation rolls back none of the callback's changes.
    pub fn set_theme_instance(&self, theme: Rc<Theme>) -> Result<(), ThemeError> {
        self.publish(theme);
        self.select(IN_MEMORY);
        self.stop_theme_watcher();
        self.notify()
    }

    /// Replace the single change callback.
    pub fn on_theme_change(&self, callback: Callback) {
        let previous = self.lifecycle.callback.borrow_mut().replace(callback);
        drop(previous);
    }

    /// Replace the published theme.
    pub(super) fn publish(&self, theme: Rc<Theme>) {
        *self.lifecycle.live.borrow_mut() = Some(theme);
    }

    /// Select the theme name.
    pub(super) fn select(&self, name: &str) {
        *self.lifecycle.name.borrow_mut() = Some(name.to_owned());
    }

    /// Invoke the callback outside every state borrow.
    pub(super) fn notify(&self) -> Result<(), ThemeError> {
        let callback = self.lifecycle.callback.borrow().clone();
        callback.map_or(Ok(()), |callback| {
            callback().map_err(ThemeError::from_callback)
        })
    }

    /// Publish the `dark` theme; no watcher is started or replaced.
    fn fall_back(&self) -> Result<(), ThemeError> {
        self.select("dark");
        self.load_named("dark").map(|theme| self.publish(theme))
    }

    /// Select `dark` or `light` from the second field of `COLORFGBG`.
    pub(super) fn default_theme(&self) -> String {
        let value = self.operations.environment("COLORFGBG").unwrap_or_default();
        let dark = value.split(';').nth(1).is_none_or(background_is_dark);
        (if dark { "dark" } else { "light" }).to_owned()
    }
}

/// Whitespace that decimal integer parsing skips.
fn is_leading_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// Parse the leading decimal integer of a field; no digits means dark.
fn background_is_dark(field: &str) -> bool {
    let field = field.trim_start_matches(is_leading_space);
    let (negative, rest) = match field.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, field.strip_prefix('+').unwrap_or(field)),
    };
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let magnitude = rest[..digits].trim_start_matches('0');
    digits == 0 || negative || magnitude.is_empty() || (magnitude.len() == 1 && magnitude < "8")
}

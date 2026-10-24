//! Caller-owned localization with shared mutable dictionaries.
use indexmap::IndexMap;
use serde::Deserialize;
use std::{cell::RefCell, fmt, rc::Rc};

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserLanguageHost;

/// Open language identifier.
pub type LanguageCode = String;
/// Default callable signature; lookup returns its shared handle without invoking it.
pub type TranslationFunction = dyn Fn(&[String]) -> String;
/// Shared message dictionary.
pub type I18nMessages<F = TranslationFunction> = Rc<RefCell<IndexMap<String, TranslationValue<F>>>>;
/// Shared language dictionary.
pub type Translations<F = TranslationFunction> =
    Rc<RefCell<IndexMap<LanguageCode, I18nMessages<F>>>>;
/// A text, shared category or shared caller-defined callable.
pub enum TranslationValue<F: ?Sized = TranslationFunction> {
    /// Literal message.
    Text(String),
    /// Nested dictionary.
    Category(I18nMessages<F>),
    /// Callable returned unchanged.
    Function(Rc<F>),
}
impl<F: ?Sized> Clone for TranslationValue<F> {
    fn clone(&self) -> Self {
        match self {
            Self::Text(s) => Self::Text(s.clone()),
            Self::Category(c) => Self::Category(Rc::clone(c)),
            Self::Function(f) => Self::Function(Rc::clone(f)),
        }
    }
}
/// Missing lookup notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum I18nDiagnostic {
    /// Missing or empty message.
    MissingKey {
        /// Optional containing category.
        category: Option<String>,
        /// Requested key.
        key: String,
    },
    /// Absent or non-category value.
    MissingCategory {
        /// Requested category.
        category: String,
    },
}
/// Localization failure.
#[derive(Debug)]
pub enum I18nError {
    /// Embedded dictionary decoding failed.
    Data(serde_json::Error),
    /// Browser operation failed.
    Host(String),
    /// Neither the selected nor English dictionary exists.
    MissingLanguage(LanguageCode),
    /// A textual consumer received a category or callable.
    NotText(String),
    /// Caller retains an incompatible dictionary borrow.
    BorrowConflict,
}
impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Data(error) => error.fmt(f),
            Self::Host(error) => f.write_str(error),
            Self::MissingLanguage(code) => write!(f, "Missing language dictionary: {code}"),
            Self::NotText(key) => write!(f, "Translation is not text: {key}"),
            Self::BorrowConflict => f.write_str("Translation dictionary is already borrowed"),
        }
    }
}
impl std::error::Error for I18nError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Data(error) => Some(error),
            _ => None,
        }
    }
}
/// Supplied browser language effects.
pub trait LanguageHost {
    /// Read persisted selection.
    /// # Errors
    /// Returns the host read failure.
    fn stored_language(&self) -> Result<Option<String>, I18nError>;
    /// Read browser locale.
    /// # Errors
    /// Returns the host read failure.
    fn browser_language(&self) -> Result<Option<String>, I18nError>;
    /// Read legacy locale when browser locale is absent or empty.
    /// # Errors
    /// Returns the host read failure.
    fn legacy_language(&self) -> Result<Option<String>, I18nError>;
    /// Persist the supplied spelling.
    /// # Errors
    /// Returns the host write failure.
    fn store_language(&self, code: &str) -> Result<(), I18nError>;
    /// Reload the page.
    /// # Errors
    /// Returns the host reload failure.
    fn reload(&self) -> Result<(), I18nError>;
    /// Report a missing entry.
    fn unknown_translation(&self, diagnostic: &I18nDiagnostic);
}
/// Shipped text-only data.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Assets {
    /// Resolved application messages.
    tables: IndexMap<String, IndexMap<String, String>>,
    /// Original dependency defaults.
    defaults: IndexMap<String, IndexMap<String, String>>,
}
/// Wrap literal entries in a shared dictionary.
fn messages<F: ?Sized>(data: IndexMap<String, String>) -> I18nMessages<F> {
    Rc::new(RefCell::new(
        data.into_iter()
            .map(|(k, v)| (k, TranslationValue::Text(v)))
            .collect(),
    ))
}
/// A localization context retaining its original and current tables.
pub struct I18n<F: ?Sized = TranslationFunction> {
    /// Ambient language adapter.
    host: Rc<dyn LanguageHost>,
    /// Original English defaults.
    english: I18nMessages<F>,
    /// Original German defaults.
    german: I18nMessages<F>,
    /// Original exported application table.
    original: Translations<F>,
    /// Currently selected dictionaries.
    current: Translations<F>,
}
impl<F: ?Sized> I18n<F> {
    /// Resolve a textual consumer without flattening categories or callables.
    pub(crate) fn text(&self, key: &str) -> Result<String, I18nError> {
        match self.i18n(key, None)? {
            TranslationValue::Text(text) => Ok(text),
            _ => Err(I18nError::NotText(key.into())),
        }
    }

    /// Decode shipped tables for this context.
    /// # Errors
    /// Returns an embedded asset decoding failure.
    pub fn new(host: Rc<dyn LanguageHost>) -> Result<Self, I18nError> {
        let assets: Assets = serde_json::from_str(include_str!("../../assets/i18n.json"))
            .map_err(I18nError::Data)?;
        let mut defaults = assets.defaults;
        let english = messages(
            defaults
                .shift_remove("en")
                .ok_or_else(|| I18nError::MissingLanguage("en".into()))?,
        );
        let german = messages(
            defaults
                .shift_remove("de")
                .ok_or_else(|| I18nError::MissingLanguage("de".into()))?,
        );
        let original = Rc::new(RefCell::new(
            assets
                .tables
                .into_iter()
                .map(|(k, v)| (k, messages(v)))
                .collect(),
        ));
        Ok(Self {
            host,
            english,
            german,
            current: Rc::clone(&original),
            original,
        })
    }
    /// Return an alias of the original English defaults.
    #[must_use]
    pub fn default_english(&self) -> I18nMessages<F> {
        Rc::clone(&self.english)
    }
    /// Return an alias of the original German defaults.
    #[must_use]
    pub fn default_german(&self) -> I18nMessages<F> {
        Rc::clone(&self.german)
    }
    /// Return an alias of the original application dictionaries.
    #[must_use]
    pub fn translations(&self) -> Translations<F> {
        Rc::clone(&self.original)
    }
    /// Replace current dictionaries without retargeting existing aliases.
    pub fn set_translations(&mut self, translations: Translations<F>) {
        self.current = translations;
    }
    /// Return an alias of the current dictionaries.
    #[must_use]
    pub fn get_translations(&self) -> Translations<F> {
        Rc::clone(&self.current)
    }
    /// Select nonempty supported storage, otherwise the locale's hyphen prefix, otherwise English.
    /// # Errors
    /// Propagates required host reads or incompatible table borrows.
    pub fn get_current_language(&self) -> Result<LanguageCode, I18nError> {
        if let Some(stored) = self.host.stored_language()?.filter(|s| !s.is_empty())
            && self.supports(&stored)?
        {
            return Ok(stored);
        }
        let locale = match self.host.browser_language()?.filter(|s| !s.is_empty()) {
            Some(locale) => Some(locale),
            None => self.host.legacy_language()?,
        };
        let code = locale
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("en")
            .split('-')
            .next()
            .unwrap_or("en");
        Ok(if self.supports(code)? { code } else { "en" }.to_owned())
    }
    /// Check exact dictionary-key membership.
    fn supports(&self, code: &str) -> Result<bool, I18nError> {
        Ok(self
            .current
            .try_borrow()
            .map_err(|_| I18nError::BorrowConflict)?
            .contains_key(code))
    }
    /// Write the language before requesting reload.
    /// # Errors
    /// A write failure prevents reload; a reload failure leaves the write in place.
    pub fn set_language(&self, code: &str) -> Result<(), I18nError> {
        self.host.store_language(code)?;
        self.host.reload()
    }
    /// Resolve a flat or nested entry, returning the requested key for absent or empty text.
    /// # Errors
    /// Propagates language selection, missing dictionary and borrow failures.
    pub fn i18n(
        &self,
        category_or_key: &str,
        key: Option<&str>,
    ) -> Result<TranslationValue<F>, I18nError> {
        let language = self.get_current_language()?;
        let table = {
            let tables = self
                .current
                .try_borrow()
                .map_err(|_| I18nError::BorrowConflict)?;
            tables
                .get(&language)
                .or_else(|| tables.get("en"))
                .cloned()
                .ok_or(I18nError::MissingLanguage(language))?
        };
        if let Some(key) = key {
            let category = {
                let messages = table.try_borrow().map_err(|_| I18nError::BorrowConflict)?;
                match messages.get(category_or_key) {
                    Some(TranslationValue::Category(category)) => Some(category.clone()),
                    _ => None,
                }
            };
            let Some(category) = category else {
                self.host
                    .unknown_translation(&I18nDiagnostic::MissingCategory {
                        category: category_or_key.to_owned(),
                    });
                return Ok(TranslationValue::Text(key.to_owned()));
            };
            let value = category
                .try_borrow()
                .map_err(|_| I18nError::BorrowConflict)?
                .get(key)
                .cloned();
            Ok(self.resolve(value, Some(category_or_key), key))
        } else {
            let value = table
                .try_borrow()
                .map_err(|_| I18nError::BorrowConflict)?
                .get(category_or_key)
                .cloned();
            Ok(self.resolve(value, None, category_or_key))
        }
    }
    /// Report absent or empty leaves after dictionary borrows have ended.
    fn resolve(
        &self,
        value: Option<TranslationValue<F>>,
        category: Option<&str>,
        key: &str,
    ) -> TranslationValue<F> {
        match value {
            Some(TranslationValue::Text(ref s)) if s.is_empty() => {}
            Some(value) => return value,
            None => {}
        }
        self.host.unknown_translation(&I18nDiagnostic::MissingKey {
            category: category.map(str::to_owned),
            key: key.to_owned(),
        });
        TranslationValue::Text(key.to_owned())
    }
}

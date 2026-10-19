//! Controlled language effects at the public host boundary.
#![cfg(test)]
use maestro_web::{I18nDiagnostic, I18nError, LanguageHost};
use std::cell::RefCell;

/// Host state with an ordered effect record.
#[derive(Default)]
pub struct Host {
    /// Stored selection.
    pub stored: RefCell<Option<String>>,
    /// Browser locale.
    pub locale: RefCell<Option<String>>,
    /// Legacy locale.
    pub legacy: RefCell<Option<String>>,
    /// Effect that must fail.
    pub fail: RefCell<String>,
    /// Observed effects.
    pub effects: RefCell<Vec<String>>,
    /// Missing-entry notifications.
    pub diagnostics: RefCell<Vec<I18nDiagnostic>>,
}
impl Host {
    /// Observe an effect before deciding whether it fails.
    fn effect(&self, name: &str) -> Result<(), I18nError> {
        self.effects.borrow_mut().push(name.to_owned());
        if *self.fail.borrow() == name {
            Err(I18nError::Host(format!("{name} failed")))
        } else {
            Ok(())
        }
    }
}
impl LanguageHost for Host {
    fn stored_language(&self) -> Result<Option<String>, I18nError> {
        self.effect("get")?;
        Ok(self.stored.borrow().clone())
    }
    fn browser_language(&self) -> Result<Option<String>, I18nError> {
        self.effect("locale")?;
        Ok(self.locale.borrow().clone())
    }
    fn legacy_language(&self) -> Result<Option<String>, I18nError> {
        self.effect("legacy")?;
        Ok(self.legacy.borrow().clone())
    }
    fn store_language(&self, code: &str) -> Result<(), I18nError> {
        self.effect("set")?;
        *self.stored.borrow_mut() = Some(code.to_owned());
        Ok(())
    }
    fn reload(&self) -> Result<(), I18nError> {
        self.effect("reload")
    }
    fn unknown_translation(&self, diagnostic: &I18nDiagnostic) {
        self.diagnostics.borrow_mut().push(diagnostic.clone());
    }
}

/// Embedded brand input without filesystem operations.
#[cfg(target_arch = "wasm32")]
pub(crate) use crate::brand;

/// Awaited screenshot observations for native reading witnesses.
#[cfg(target_arch = "wasm32")]
pub mod reading_paint;

/// Shared projection for native reading consumers.
#[cfg(target_arch = "wasm32")]
pub mod reading_view;

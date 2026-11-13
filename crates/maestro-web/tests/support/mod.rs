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
pub(crate) use crate::brand;

/// Awaited screenshot observations for native reading witnesses.
#[cfg(target_arch = "wasm32")]
pub mod reading_paint;

/// Primary-button pointer input shared by canvas witnesses.
pub mod pointer;

/// Shared recorded tool operands.
pub mod tool_input;

/// Check a fixture record's strict fields without changing its operation payload.
/// # Panics
/// Panics if the valid record fails or a malformed record is accepted.
pub fn strict_record<T: serde::de::DeserializeOwned>(valid: &serde_json::Value, required: &[&str]) {
    use serde_json::{Value, json};
    assert!(serde_json::from_value::<T>(valid.clone()).is_ok());
    for value in [Value::Null, json!([])] {
        let mut extra = valid.clone();
        extra["unexpected"] = value;
        assert!(serde_json::from_value::<T>(extra).is_err());
    }
    for &field in required {
        let mut missing = valid.clone();
        missing
            .as_object_mut()
            .unwrap()
            .shift_remove(field)
            .unwrap();
        assert!(
            serde_json::from_value::<T>(missing).is_err(),
            "missing {field}"
        );
        let wrong = if valid[field].is_object() || valid[field].is_array() {
            json!(false)
        } else {
            json!([])
        };
        for value in [Value::Null, wrong] {
            let mut invalid = valid.clone();
            invalid[field] = value;
            assert!(
                serde_json::from_value::<T>(invalid).is_err(),
                "invalid {field}"
            );
        }
    }
}
/// Check that a fixture recording rejects sections owned by another reader group.
/// # Panics
/// Panics if the recording admits one of the foreign sections.
pub fn reject_sections<T: serde::de::DeserializeOwned>(
    valid: &serde_json::Value,
    sections: &[&str],
) {
    for value in [serde_json::Value::Null, serde_json::json!([])] {
        for &section in sections {
            let mut foreign = valid.clone();
            foreign[section] = value.clone();
            assert!(
                serde_json::from_value::<T>(foreign).is_err(),
                "accepted {section}"
            );
        }
    }
}

/// Browser query helpers for the widget witnesses.
#[cfg(target_arch = "wasm32")]
pub mod dom {
    use wasm_bindgen::JsCast as _;

    /// Every matching descendant in document order.
    ///
    /// # Panics
    /// Panics if the selector is not a valid query.
    #[must_use]
    pub fn all(root: &web_sys::Element, selector: &str) -> Vec<web_sys::Element> {
        let nodes = root.query_selector_all(selector).unwrap();
        (0..nodes.length())
            .map(|index| nodes.item(index).unwrap().unchecked_into())
            .collect()
    }
}

/// Controlled message records decoded into the caller's browser record.
#[cfg(target_arch = "wasm32")]
pub mod agent_records {
    /// A user entry saying `text`, decoded into the caller's record.
    ///
    /// # Panics
    /// Panics if the record is malformed.
    #[must_use]
    pub fn user_entry<T: serde::de::DeserializeOwned>(text: &str) -> T {
        serde_json::from_value(serde_json::json!({"role": "user", "content": text, "timestamp": 1}))
            .unwrap()
    }
}

/// Complete `future` on a fresh current-thread runtime without I/O or time drivers.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

#[cfg(not(target_arch = "wasm32"))]
mod runtime;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use runtime::run_io;

pub(crate) mod attachment_observation;
pub(crate) mod attachments;
pub(crate) mod browser_records;
pub(crate) mod browser_storage_records;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod document_attachments;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod document_extraction;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod javascript_execution;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod pdf;
#[cfg(not(target_arch = "wasm32"))]
mod shared_operands;
pub(crate) mod spreadsheet;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod spreadsheet_native;

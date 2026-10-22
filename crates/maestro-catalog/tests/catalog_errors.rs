#![cfg(test)]
//! Public catalog diagnostic ownership.
use maestro_catalog::CatalogError;
use maestro_models::{DiagnosticCode, DiagnosticErrorInfo};
use std::error::Error;

#[test]
fn catalog_diagnostics_keep_fields_without_account_source() {
    for (message, name, code) in [
        ("plain", None, None),
        ("", Some("Controlled"), Some(DiagnosticCode::Number(7.0))),
        (
            "provider failed: é\nsecond line",
            Some("Controlled"),
            Some(DiagnosticCode::Text("EINNER".into())),
        ),
    ] {
        let diagnostic = DiagnosticErrorInfo {
            message: message.into(),
            name: name.map(String::from),
            stack: name.map(|_| "supplied stack".into()),
            code,
        };
        let expected = diagnostic.clone();
        let error = CatalogError::from(diagnostic);
        let CatalogError::Diagnostic(retained) = &error else {
            panic!("expected catalog diagnostic")
        };
        assert!(std::ptr::eq(error.diagnostic(), retained.as_ref()));
        assert_eq!(error.diagnostic(), &expected);
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none());
    }
}

#[test]
fn account_failures_keep_original_diagnostic_and_source_chain() {
    for (message, rich) in [
        ("", false),
        ("plain", false),
        ("", true),
        ("provider failed: é\nsecond line", true),
    ] {
        let original = if rich {
            dynamic::account_failure(message)
        } else {
            maestro_models::OAuthError::from(DiagnosticErrorInfo {
                message: message.into(),
                name: None,
                stack: None,
                code: None,
            })
        };
        let address = std::ptr::from_ref(original.diagnostic.as_ref());
        let expected = original.diagnostic.as_ref().clone();
        let error = CatalogError::from(original);
        assert_eq!(std::ptr::from_ref(error.diagnostic()), address);
        let account = error
            .source()
            .unwrap()
            .downcast_ref::<maestro_models::OAuthError>()
            .unwrap();
        assert_eq!(std::ptr::from_ref(account.diagnostic.as_ref()), address);
        if rich {
            dynamic::assert_account_failure(&error, message);
        } else {
            assert!(matches!(error, CatalogError::OAuth(_)));
            assert_eq!(error.diagnostic(), &expected);
            assert_eq!(error.to_string(), message);
            assert!(account.errno.is_none());
            assert!(account.source().is_none());
        }
    }
}

use crate::dynamic;

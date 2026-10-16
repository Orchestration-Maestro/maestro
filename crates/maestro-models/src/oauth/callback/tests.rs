//! Controlled manual-input publication and ownership witnesses.
use super::start_manual;
use crate::oauth::native::CallbackOutcome;
use crate::{DiagnosticCode, DiagnosticErrorInfo, EventStream, OAuthError};

/// Drive owned manual work on the same single-thread executor as its observers.
fn run(work: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(work);
}

#[test]
fn manual_completion_preserves_supplied_outcome() {
    run(async {
        for supplied in [
            "",
            "  supplied  ",
            "\u{feff}raw\u{85}",
            "code=m&state=",
            "https://example.invalid/callback?code=manual-code&state=verifier",
            "supplied-🦀",
        ] {
            let wait = EventStream::new(|_| true, |_| ());
            let manual = start_manual(Box::pin(async move { Ok(supplied.into()) }), wait.clone());
            assert!(matches!(
                wait.next().await,
                Some(CallbackOutcome::Cancelled)
            ));
            assert_eq!(manual.next().await.unwrap().unwrap(), supplied);
            assert!(manual.next().await.is_none());
            assert!(wait.next().await.is_none());
        }
        let mut error: OAuthError = DiagnosticErrorInfo {
            message: "manual failed".into(),
            name: Some("ManualInput".into()),
            code: Some(DiagnosticCode::Number(7.0)),
            stack: Some("supplied stack".into()),
        }
        .into();
        error.errno = Some(DiagnosticCode::Number(9.0));
        error.cause = Some(Box::new(OAuthError::message("supplied cause")));
        let diagnostic = std::ptr::from_ref(&*error.diagnostic);
        let cause = std::ptr::from_ref(error.cause.as_deref().unwrap());
        let wait = EventStream::new(|_| true, |_| ());
        let manual = start_manual(Box::pin(async move { Err(error) }), wait.clone());
        assert!(matches!(
            wait.next().await,
            Some(CallbackOutcome::Cancelled)
        ));
        let error = manual.next().await.unwrap().unwrap_err();
        assert!(std::ptr::eq(
            std::ptr::from_ref(&*error.diagnostic),
            diagnostic
        ));
        assert!(std::ptr::eq(error.cause.as_deref().unwrap(), cause));
        assert_eq!(error.diagnostic.message, "manual failed");
        assert_eq!(error.diagnostic.name.as_deref(), Some("ManualInput"));
        assert_eq!(error.diagnostic.code, Some(DiagnosticCode::Number(7.0)));
        assert_eq!(error.diagnostic.stack.as_deref(), Some("supplied stack"));
        assert_eq!(error.errno, Some(DiagnosticCode::Number(9.0)));
        assert_eq!(
            std::error::Error::source(&error).unwrap().to_string(),
            "supplied cause"
        );
        assert!(manual.next().await.is_none());
        assert!(wait.next().await.is_none());
    });
}

#[test]
fn manual_work_survives_dropped_observers() {
    run(async {
        for failure in [false, true] {
            let wait = EventStream::new(|_| true, |_| ());
            wait.push(CallbackOutcome::Accepted(super::AuthorizationCode {
                code: Some("browser-code".into()),
                state: Some("verifier".into()),
            }));
            let Some(CallbackOutcome::Accepted(fields)) = wait.next().await else {
                panic!("expected accepted browser input");
            };
            assert_eq!(fields.code.as_deref(), Some("browser-code"));
            let release = EventStream::new(|()| true, |()| ());
            let gate = release.clone();
            let completed = EventStream::new(|()| true, |()| ());
            let witness = completed.clone();
            let result = if failure {
                Err(OAuthError::message("late failure"))
            } else {
                Ok("late success".into())
            };
            let manual = start_manual(
                Box::pin(async move {
                    gate.result().await;
                    witness.push(());
                    result
                }),
                wait.clone(),
            );
            drop(manual);
            drop(wait);
            release.push(());
            completed.result().await;
        }
    });
}

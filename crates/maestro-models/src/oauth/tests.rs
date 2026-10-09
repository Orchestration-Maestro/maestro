//! Numeric expiry and lazy clock selection through the production operation.
#![cfg(test)]
use super::*;
use std::sync::{Mutex, MutexGuard};

/// Serialize observations of this binary's registry.
static ADMISSION: Mutex<()> = Mutex::new(());
/// Reset membership after observations, including unwinding.
struct Admission {
    /// Retain admission until cleanup.
    _guard: MutexGuard<'static, ()>,
}
impl Admission {
    /// Prepare an isolated registry.
    fn enter() -> Self {
        let guard = ADMISSION
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reset_oauth_providers();
        Self { _guard: guard }
    }
}
impl Drop for Admission {
    fn drop(&mut self) {
        reset_oauth_providers();
    }
}
/// Provider recording completed selection phases.
struct Provider {
    /// Ordered phase observations.
    events: Arc<Mutex<Vec<&'static str>>>,
}
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &'static str {
        "controlled"
    }
    fn name(&self) -> &'static str {
        "controlled"
    }
    fn login(
        &self,
        _: OAuthCallbacks,
        _: Option<crate::Fetch>,
    ) -> crate::BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { panic!("registry does not authorize") })
    }
    fn refresh_token(
        &self,
        mut record: OAuthCredentials,
        _: Option<crate::Fetch>,
    ) -> crate::BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.events.lock().unwrap().push("refresh");
        record.access = "new-access".into();
        record.refresh = "new-refresh".into();
        record.expires = f64::NAN;
        Box::pin(async move { Ok(record) })
    }
    fn get_api_key<'a>(&self, record: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        self.events.lock().unwrap().push("key");
        Ok(&record.refresh)
    }
}
/// Supplied record with distinguishable fields and retained extension data.
fn record(expires: f64) -> OAuthCredentials {
    OAuthCredentials {
        refresh: "old-refresh".into(),
        access: "old-access".into(),
        expires,
        extra: crate::JsonObject::from_iter([(
            "nested".into(),
            serde_json::json!({"value": [2, "kept"]}),
        )]),
    }
}
/// Drive the controlled operation to completion.
fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
/// Assert one numeric decision and the resulting credential selection.
fn check_expiry(now: f64, expiry: f64, refresh: bool, events: &Arc<Mutex<Vec<&'static str>>>) {
    events.lock().unwrap().clear();
    let input = IndexMap::from([("controlled".into(), record(expiry))]);
    let result = run(get_oauth_api_key_with_clock(
        "controlled",
        &input,
        None,
        || {
            events.lock().unwrap().push("clock");
            now
        },
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        result.api_key,
        if refresh {
            "new-refresh"
        } else {
            "old-refresh"
        }
    );
    assert_eq!(
        result.new_credentials.access,
        if refresh { "new-access" } else { "old-access" }
    );
    assert_eq!(result.new_credentials.extra, input["controlled"].extra);
    if refresh || expiry.is_nan() {
        assert!(result.new_credentials.expires.is_nan());
    } else {
        assert_eq!(result.new_credentials.expires.to_bits(), expiry.to_bits());
    }
    assert_eq!(input["controlled"].expires.to_bits(), expiry.to_bits());
    assert_eq!(input["controlled"].access, "old-access");
    assert_eq!(
        *events.lock().unwrap(),
        if refresh {
            vec!["clock", "refresh", "key"]
        } else {
            vec!["clock", "key"]
        }
    );
}

#[test]
fn maestro_oauth_expiry_uses_numeric_comparison() {
    let _admission = Admission::enter();
    let events = Arc::new(Mutex::new(Vec::new()));
    register_oauth_provider(Arc::new(Provider {
        events: Arc::clone(&events),
    }));
    let subnormal = f64::from_bits(1);
    for (expiry, refresh) in [
        (999.75, true),
        (1000.0, true),
        (1000.25, false),
        (0.0, true),
        (-0.0, true),
        (-1.0, true),
        (f64::NAN, false),
        (f64::INFINITY, false),
        (f64::NEG_INFINITY, true),
        (f64::MAX, false),
        (subnormal, true),
        (9_007_199_254_740_992.0, false),
    ] {
        check_expiry(1000.0, expiry, refresh, &events);
    }
    for (expiry, refresh) in [
        (-subnormal, true),
        (-0.0, true),
        (0.0, true),
        (subnormal, false),
    ] {
        check_expiry(0.0, expiry, refresh, &events);
    }
}

#[test]
fn maestro_oauth_clock_is_read_only_for_present_credentials() {
    let _admission = Admission::enter();
    let events = Arc::new(Mutex::new(Vec::new()));
    register_oauth_provider(Arc::new(Provider {
        events: Arc::clone(&events),
    }));
    let empty = IndexMap::new();
    let clock = || {
        events.lock().unwrap().push("clock");
        1000.0
    };
    assert_eq!(
        run(get_oauth_api_key_with_clock("unknown", &empty, None, clock))
            .err()
            .unwrap()
            .to_string(),
        "Unknown OAuth provider: unknown"
    );
    assert!(
        run(get_oauth_api_key_with_clock(
            "controlled",
            &empty,
            None,
            clock
        ))
        .unwrap()
        .is_none()
    );
    assert!(events.lock().unwrap().is_empty());
    check_present_clock(&events);
    events.lock().unwrap().clear();
    let output = run(refresh_oauth_token(
        "controlled",
        record(f64::INFINITY),
        None,
    ))
    .unwrap();
    assert_eq!(output.access, "new-access");
    assert_eq!(*events.lock().unwrap(), ["refresh"]);
}

/// Present credentials read the clock before their selected callback phases.
fn check_present_clock(events: &Arc<Mutex<Vec<&'static str>>>) {
    let clock = || {
        events.lock().unwrap().push("clock");
        1000.0
    };
    for (expiry, refresh) in [(999.0, true), (1001.0, false)] {
        events.lock().unwrap().clear();
        let input = IndexMap::from([("controlled".into(), record(expiry))]);
        let output = run(get_oauth_api_key_with_clock(
            "controlled",
            &input,
            None,
            clock,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            output.api_key,
            if refresh {
                "new-refresh"
            } else {
                "old-refresh"
            }
        );
        assert_eq!(
            *events.lock().unwrap(),
            if refresh {
                vec!["clock", "refresh", "key"]
            } else {
                vec!["clock", "key"]
            }
        );
    }
}

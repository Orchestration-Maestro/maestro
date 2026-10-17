//! Owned-source refresh boundaries, token rotation and shared in-flight acquisition.

use super::super::base_external_client::Effects;
use google_cloud_auth::credentials::{CacheableResource, Credentials};
use google_cloud_auth::errors::CredentialsError;
use num_traits::ToPrimitive as _;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

/// Requests seen by a scripted effect.
type Seen = Arc<Mutex<Vec<crate::HttpRequest>>>;

/// A controlled clock and the effects built over it.
pub(super) fn effects(send: crate::Fetch, clock: &Arc<AtomicU64>) -> Effects {
    let clock = clock.clone();
    Effects {
        fetch: send,
        clock: Arc::new(move || millis(&clock)),
        ..Effects::default()
    }
}

/// Current controlled time in milliseconds.
fn millis(clock: &AtomicU64) -> f64 {
    clock.load(Ordering::SeqCst).to_f64().unwrap()
}

/// Authorization value generated for the next use.
pub(super) async fn bearer(credentials: &Credentials) -> Result<String, CredentialsError> {
    match credentials.headers(hyper::http::Extensions::new()).await? {
        CacheableResource::New { data, .. } => Ok(data
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()),
        CacheableResource::NotModified => panic!("expected generated headers"),
    }
}

/// Respond with each JSON body in turn (the last repeats), recording requests.
pub(super) fn script(bodies: Vec<serde_json::Value>) -> (crate::Fetch, Seen) {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let record = seen.clone();
    let send: crate::Fetch = Arc::new(move |request| {
        let mut seen = record.lock().unwrap();
        let body = bodies[seen.len().min(bodies.len() - 1)].clone();
        seen.push(request);
        Box::pin(std::future::ready(Ok(super::response(&body))))
    });
    (send, seen)
}

/// A token response with a relative lifetime in seconds.
pub(super) fn grant(access: &str, expires_in: i64) -> serde_json::Value {
    serde_json::json!({"access_token":access,"expires_in":expires_in})
}

/// Workforce credentials over controlled effects.
pub(super) fn workforce(effects: Effects) -> Credentials {
    super::super::google_auth::from_json(
        super::workforce("https://auth.invalid/token", "quota"),
        effects,
    )
    .unwrap()
}

#[test]
fn workforce_refresh_emits_ordered_encoded_form() {
    super::run(async {
        let (send, seen) = script(vec![grant("a", 3600)]);
        let value = serde_json::json!({"type":"external_account_authorized_user","client_id":"id é","client_secret":"p:w","refresh_token":"a b+c&d😀","token_url":"https://auth.invalid/token"});
        let clock = Arc::new(AtomicU64::new(1000));
        let credentials =
            super::super::google_auth::from_json(value, effects(send, &clock)).unwrap();
        bearer(&credentials).await.unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(
            String::from_utf8(seen[0].body.clone()).unwrap(),
            "grant_type=refresh_token&refresh_token=a+b%2Bc%26d%F0%9F%98%80"
        );
        assert_eq!(seen[0].headers["authorization"], "Basic aWQgw6k6cDp3");
        assert_eq!(
            seen[0].headers["content-type"],
            "application/x-www-form-urlencoded"
        );
        assert_eq!(seen[0].method, "POST");
    });
}

#[test]
fn workforce_refresh_retains_rotated_refresh_token() {
    super::run(async {
        let rotated = |token: &str, access: &str, expires: i64| {
            let mut body = grant(access, expires);
            body["refresh_token"] = token.into();
            body
        };
        let (send, seen) = script(vec![
            rotated("rotated-one", "first", 3600),
            grant("second", 3600),
            rotated("rotated-two", "expired", 0),
            grant("third", 3600),
        ]);
        let clock = Arc::new(AtomicU64::new(1000));
        let credentials = workforce(effects(send, &clock));
        bearer(&credentials).await.unwrap();
        clock.store(10_000_000, Ordering::SeqCst);
        bearer(&credentials).await.unwrap();
        clock.store(20_000_000, Ordering::SeqCst);
        assert!(bearer(&credentials).await.is_err());
        bearer(&credentials).await.unwrap();
        let tokens: Vec<_> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|request| {
                url::form_urlencoded::parse(&request.body)
                    .find(|(name, _)| name == "refresh_token")
                    .unwrap()
                    .1
                    .into_owned()
            })
            .collect();
        assert_eq!(
            tokens,
            [
                "controlled-refresh",
                "rotated-one",
                "rotated-one",
                "rotated-two"
            ]
        );
    });
}

/// Metadata credentials over a scripted effect and the shared controlled clock.
fn compute_credentials(clock: &Arc<AtomicU64>) -> (Credentials, Seen) {
    let (send, seen) = script(vec![grant("controlled-access", 3600)]);
    let wrapped: crate::Fetch = Arc::new(move |request| {
        let future = send(request);
        Box::pin(async move {
            let mut response = future.await?;
            response
                .headers
                .insert("metadata-flavor".into(), "Google".into());
            Ok(response)
        })
    });
    let metadata = super::super::metadata::Metadata::new(
        Some("assume-present"),
        false,
        (None, Some("controlled.invalid")),
        0,
    )
    .unwrap();
    let source = super::super::compute_client::Compute::new(metadata);
    let credentials =
        Credentials::from(super::super::base_external_client::BaseExternalClient::new(
            source,
            effects(wrapped, clock),
            &serde_json::json!({}),
        ));
    (credentials, seen)
}

#[test]
fn custom_credentials_refresh_at_the_demand_boundary() {
    super::run(async {
        let clock = Arc::new(AtomicU64::new(1000));
        let (send, seen) = script(vec![grant("controlled-access", 3600)]);
        let workforce = workforce(effects(send, &clock));
        let (compute, compute_seen) = compute_credentials(&clock);
        let files = super::Files::new();
        let (server, received) = super::certificates::tls_server(super::certificates::granted);
        let reader = clock.clone();
        let certificate = super::certificates::credentials(
            &server,
            &files,
            &serde_json::json!({}),
            Arc::new(move || millis(&reader)),
        );
        let expiry = 1000 + 3_600_000;
        for credentials in [&workforce, &compute, &certificate] {
            clock.store(1000, Ordering::SeqCst);
            bearer(credentials).await.unwrap();
            clock.store(expiry - 300_000 - 1, Ordering::SeqCst);
            bearer(credentials).await.unwrap();
        }
        let counts = || {
            (
                seen.lock().unwrap().len(),
                compute_seen.lock().unwrap().len(),
                received.try_iter().count(),
            )
        };
        assert_eq!(counts(), (1, 1, 1));
        clock.store(expiry - 300_000, Ordering::SeqCst);
        for credentials in [&workforce, &compute, &certificate] {
            bearer(credentials).await.unwrap();
        }
        assert_eq!(counts(), (2, 2, 1));
        drop(certificate);
        server.finish().unwrap();
    });
}

#[test]
fn custom_credentials_reuse_valid_or_unexpiring_tokens() {
    super::run(async {
        let clock = Arc::new(AtomicU64::new(1000));
        let (send, seen) = script(vec![serde_json::json!({"access_token":"unexpiring"})]);
        let unexpiring = workforce(effects(send, &clock));
        for time in [1000, 50_000_000_000, 90_000_000_000_000] {
            clock.store(time, Ordering::SeqCst);
            assert_eq!(bearer(&unexpiring).await.unwrap(), "Bearer unexpiring");
        }
        assert_eq!(seen.lock().unwrap().len(), 1);
        clock.store(1000, Ordering::SeqCst);
        for expires in [0, -5] {
            let (send, seen) = script(vec![grant("known-expired", expires)]);
            let rejected = workforce(effects(send, &clock));
            assert!(bearer(&rejected).await.is_err());
            assert_eq!(seen.lock().unwrap().len(), 1);
        }
        let (compute, _) = compute_credentials(&clock);
        assert_eq!(bearer(&compute).await.unwrap(), "Bearer controlled-access");
    });
}

#[test]
fn custom_refresh_failures_never_send_expired_tokens() {
    super::run(async {
        let clock = Arc::new(AtomicU64::new(1000));
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let send: crate::Fetch = Arc::new(move |_| {
            Box::pin(std::future::ready(
                match counter.fetch_add(1, Ordering::SeqCst) {
                    1 => {
                        let mut response = super::response(&serde_json::json!({}));
                        response.status = 400;
                        Ok(response)
                    }
                    0 => Ok(super::response(&grant("old", 3600))),
                    _ => Ok(super::response(&grant("new", 3600))),
                },
            ))
        });
        let client = super::client(workforce(effects(send, &clock)), indexmap::IndexMap::new());
        let projected = || {
            super::super::super::project_content_request(
                serde_json::json!({"model":"model","contents":"hello"}),
            )
            .unwrap()
        };
        let first = client.prepare_request(projected(), None).await.unwrap();
        assert_eq!(first.request.headers["authorization"], "Bearer old");
        clock.store(5_000_000, Ordering::SeqCst);
        assert!(client.prepare_request(projected(), None).await.is_err());
        let recovered = client.prepare_request(projected(), None).await.unwrap();
        assert_eq!(recovered.request.headers["authorization"], "Bearer new");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    });
}

#[test]
fn certificate_refresh_coalesces_concurrent_acquisition() {
    super::run(async {
        let files = super::Files::new();
        let (server, received) = super::certificates::tls_server(|index| {
            if index == 0 {
                (400, b"{}".to_vec())
            } else {
                super::certificates::granted(index)
            }
        });
        let credentials = super::certificates::credentials(
            &server,
            &files,
            &serde_json::json!({}),
            Arc::new(|| 1000.0),
        );
        let failures = futures_util::future::join_all((0..4).map(|_| bearer(&credentials))).await;
        assert!(failures.iter().all(Result::is_err));
        assert_eq!(received.try_iter().count(), 1);
        let grants = futures_util::future::join_all((0..4).map(|_| bearer(&credentials))).await;
        assert!(
            grants
                .iter()
                .all(|result| result.as_deref().ok() == Some("Bearer controlled-access"))
        );
        assert_eq!(received.try_iter().count(), 1);
        drop(credentials);
        server.finish().unwrap();
    });
}

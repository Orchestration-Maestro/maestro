//! Controlled header-arrival clock observations for source freshness boundaries.

use std::time::{Duration, UNIX_EPOCH};

#[test]
fn pac_http_freshness_preserves_directive_order() {
    let received = UNIX_EPOCH + Duration::from_secs(1_000_000);
    let mut observation = super::Observation {
        uri: url::Url::parse("http://localhost/script").unwrap(),
        status: 200,
        headers: std::collections::BTreeMap::new(),
        received,
    };
    for (control, fresh) in [
        ("max-age=10, no-cache", true),
        ("no-cache, max-age=10", false),
        ("max-age=10, no-store", true),
        ("no-store, max-age=10", false),
        ("max-age=0", false),
        ("max-age=-1", false),
        ("MAX-AGE=10", false),
        ("max-age=10seconds", true),
        ("max-age=invalid", false),
        ("must-revalidate, unknown, max-age=10", true),
        ("unknown", false),
    ] {
        observation
            .headers
            .insert("cache-control".into(), control.into());
        assert_eq!(
            super::fresh(&observation, received + Duration::from_secs(9)),
            fresh,
            "{control}"
        );
    }
    observation
        .headers
        .insert("cache-control".into(), "max-age=10".into());
    assert!(!super::fresh(
        &observation,
        received + Duration::from_secs(10)
    ));
    assert!(!super::fresh(
        &observation,
        received + Duration::from_secs(11)
    ));
    observation.headers.clear();
    assert!(!super::fresh(&observation, received));
    observation
        .headers
        .insert("expires".into(), "Wed, 21 Oct 2015 07:28:00 GMT".into());
    let expires = UNIX_EPOCH + Duration::from_mins(24_090_208);
    assert!(super::fresh(&observation, expires - Duration::from_secs(1)));
    assert!(!super::fresh(&observation, expires));
    observation
        .headers
        .insert("cache-control".into(), "unknown".into());
    assert!(!super::fresh(&observation, received));
}

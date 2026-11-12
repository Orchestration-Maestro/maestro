//! The daemon's workspace command handler and its arm in the daemon loop: server control,
//! notifications, workspace metadata and the one fallback.
#![cfg(target_os = "linux")]
use maestro_daemon::state::App;
use maestro_daemon::state::test_support::api::{
    daemon_with_workspaces, events_after, failure, request,
};
use maestro_daemon::state::test_support::daemon_api::{report, tokens, values};
use maestro_daemon::state::test_support::run;
use maestro_protocol::events::EventData;
use maestro_protocol::schema::ResponseResult;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};
/// The tokens each metadata event since `sequence` published.
fn published(app: &App, sequence: u64) -> Vec<HashMap<String, String>> {
    events_after(app, sequence)
        .into_iter()
        .map(|event| match event {
            EventData::WorkspaceMetadataUpdated { workspace } => workspace.tokens,
            other => panic!("unexpected event {other:?}"),
        })
        .collect()
}
#[test]
fn workspace_tokens_patch_clear_and_publish_current_values() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let first = json!({"summary": "reviewing auth", "jj_status": "2 changes"});
    assert_eq!(
        report(&mut app, "user:test", &first, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    let expected = values(&[("summary", "reviewing auth"), ("jj_status", "2 changes")]);
    assert_eq!(tokens(&app).unwrap(), expected);
    let second = json!({"summary": "done", "jj_status": null});
    assert_eq!(
        report(&mut app, "user:test", &second, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    assert_eq!(tokens(&app).unwrap(), values(&[("summary", "done")]));
    // Reporting the same values again changes nothing and publishes nothing.
    let same = json!({"summary": "done"});
    assert_eq!(
        report(&mut app, "user:test", &same, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    let cleared = json!({"summary": "  "});
    assert_eq!(
        report(&mut app, "user:test", &cleared, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    assert!(tokens(&app).unwrap().is_empty());
    assert_eq!(
        published(&app, 0),
        [expected, values(&[("summary", "done")]), HashMap::new()]
    );
}
#[test]
fn workspace_token_expiry_publishes_the_remaining_snapshot() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let paths = maestro_daemon::state::test_support::api::untouched_paths().unwrap();
    let lasting = json!({"lasting": "kept"});
    assert!(report(&mut app, "user:keep", &lasting, &json!({})).is_ok());
    let before = Instant::now();
    let temporary = json!({"summary": "temporary"});
    assert!(
        report(
            &mut app,
            "user:test",
            &temporary,
            &json!({"ttl_ms": 60_000})
        )
        .is_ok()
    );
    let deadline = app.state.topology.workspaces[0]
        .metadata_tokens
        .next_expiry()
        .unwrap();
    assert!(deadline >= before + Duration::from_secs(60));
    let sequence = app.event_hub().current_sequence();
    run(async {
        let mut now = before;
        let mut earlier_deadlines = 0;
        while let Some(next) = app.next_headless_loop_deadline(now, false)
            && next < deadline
        {
            // Repository refresh shares the timer but must not expire metadata early.
            app.handle_scheduled_tasks(next, &paths).await;
            assert!(published(&app, sequence).is_empty());
            assert_eq!(
                tokens(&app).unwrap(),
                values(&[("lasting", "kept"), ("summary", "temporary")])
            );
            now = next;
            earlier_deadlines += 1;
        }
        assert!(earlier_deadlines > 0);
        assert_eq!(app.next_headless_loop_deadline(now, false), Some(deadline));
        app.handle_scheduled_tasks(
            deadline.checked_sub(Duration::from_nanos(1)).unwrap(),
            &paths,
        )
        .await;
        assert!(published(&app, sequence).is_empty());
        assert_eq!(
            tokens(&app).unwrap(),
            values(&[("lasting", "kept"), ("summary", "temporary")])
        );
        app.handle_scheduled_tasks(deadline, &paths).await;
        assert_eq!(tokens(&app).unwrap(), values(&[("lasting", "kept")]));
        assert_eq!(published(&app, sequence), [values(&[("lasting", "kept")])]);
        app.handle_scheduled_tasks(deadline + Duration::from_secs(1), &paths)
            .await;
        assert_eq!(published(&app, sequence), [values(&[("lasting", "kept")])]);
    })
    .unwrap();
}
#[test]
fn token_values_strip_controls_and_empty_values_clear() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let set = json!({
        "summary": "  review\nready  ",
        "padded": "\u{85} a\u{0}b \u{85}",
        "long": "é".repeat(81),
        "controlled": format!(" \u{1b}{} ", "x".repeat(80)),
        "kept": "keep",
    });
    assert!(report(&mut app, "user:test", &set, &json!({})).is_ok());
    let expected = values(&[
        ("summary", "reviewready"),
        ("padded", "ab"),
        ("long", &"é".repeat(80)),
        ("controlled", &"x".repeat(80)),
        ("kept", "keep"),
    ]);
    assert_eq!(tokens(&app).unwrap(), expected);
    let clear = json!({"summary": null, "padded": " \n ", "long": "\u{0}\u{7}", "controlled": ""});
    assert!(report(&mut app, "user:test", &clear, &json!({})).is_ok());
    assert_eq!(tokens(&app).unwrap(), values(&[("kept", "keep")]));
}
#[test]
fn token_reports_reject_invalid_keys_and_excess_updates() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let invalid = |message: &str| failure("invalid_metadata_token", message);
    let too_many: serde_json::Map<String, Value> = (0..17)
        .map(|index| (format!("key{index}"), json!("value")))
        .collect();
    let sixteen: serde_json::Map<String, Value> = (0..16)
        .map(|index| (format!("key{index}"), json!("value")))
        .collect();
    let long_key = "x".repeat(33);
    for (patch, expected) in [
        (json!({}), invalid("missing token to set or clear")),
        (
            Value::Object(too_many),
            invalid("a metadata report may update at most 16 tokens"),
        ),
        (
            json!({"": "value"}),
            invalid("invalid metadata token key: "),
        ),
        (
            json!({"bad.name": "value"}),
            invalid("invalid metadata token key: bad.name"),
        ),
        (
            json!({"clé": "value"}),
            invalid("invalid metadata token key: clé"),
        ),
        (
            json!({&long_key: "value"}),
            invalid(&format!("invalid metadata token key: {long_key}")),
        ),
    ] {
        assert_eq!(
            report(&mut app, "user:test", &patch, &json!({"seq": 5})),
            expected,
            "{patch}"
        );
    }
    assert!(tokens(&app).unwrap().is_empty());
    assert!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .is_empty()
    );
    assert!(events_after(&app, 0).is_empty());
    let accepted = json!({"x".repeat(32): "value", "ok_key-2": "v"});
    assert!(report(&mut app, "user:test", &accepted, &json!({})).is_ok());
    assert!(report(&mut app, "user:test", &Value::Object(sixteen), &json!({})).is_ok());
    assert_eq!(tokens(&app).unwrap().len(), 18);
}
#[test]
fn metadata_source_and_ttl_boundaries_keep_error_order() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let token = json!({"summary": "x"});
    let source = |message: &str| failure("invalid_metadata_source", message);
    let ttl = |message: &str| failure("invalid_metadata_ttl", message);
    let ascii = "metadata source may contain only ASCII letters, digits, colon, dot, underscore, and hyphen";
    for (reporter, extra, expected) in [
        ("", json!({}), source("metadata source must not be empty")),
        (
            " \u{85} ",
            json!({}),
            source("metadata source must not be empty"),
        ),
        (
            "s".repeat(81).as_str(),
            json!({}),
            source("metadata source must be 80 characters or fewer"),
        ),
        ("user/test", json!({}), source(ascii)),
        ("usér", json!({}), source(ascii)),
        ("\u{feff}user", json!({}), source(ascii)),
        (
            "user:test",
            json!({"ttl_ms": 0}),
            ttl("metadata ttl_ms must be at least 1"),
        ),
        (
            "user:test",
            json!({"ttl_ms": 86_400_001}),
            ttl("metadata ttl_ms must be 86400000 or less"),
        ),
        // The source is checked before the lifetime, the lifetime before the tokens.
        (
            "",
            json!({"ttl_ms": 0}),
            source("metadata source must not be empty"),
        ),
    ] {
        assert_eq!(
            report(&mut app, reporter, &token, &extra),
            expected,
            "{reporter:?} {extra}"
        );
    }
    assert_eq!(
        report(&mut app, "user:test", &json!({}), &json!({"ttl_ms": 0})),
        ttl("metadata ttl_ms must be at least 1")
    );
    // An unknown workspace wins over every other fault.
    let mut missing = json!({"type": "workspace.report_metadata", "workspace_id": "w9", "source": "", "tokens": {}});
    assert_eq!(
        request(&mut app, missing.clone()),
        failure("workspace_not_found", "workspace w9 not found")
    );
    missing["workspace_id"] = json!("w_9");
    assert_eq!(
        request(&mut app, missing),
        source("metadata source must not be empty")
    );
    assert!(tokens(&app).unwrap().is_empty());
    assert_boundary_sources_and_lifetimes_apply(&mut app, &token);
}
/// Sources and lifetimes on the accepted side of each boundary apply without tracking a
/// sequence.
fn assert_boundary_sources_and_lifetimes_apply(app: &mut App, token: &Value) {
    for (reporter, extra) in [
        ("s".repeat(80).as_str(), json!({})),
        ("\u{85}a:b.c_d-e\u{85}", json!({"ttl_ms": null})),
        ("user:test", json!({"ttl_ms": 1})),
        ("user:test", json!({"ttl_ms": 86_400_000})),
    ] {
        assert_eq!(
            report(app, reporter, token, &extra),
            Ok(ResponseResult::Ok {}),
            "{reporter:?}"
        );
    }
    assert!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .is_empty()
    );
}
#[test]
fn metadata_capacity_and_sequences_are_transactional() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let full: serde_json::Map<String, Value> = (0..16)
        .map(|index| (format!("a{index}"), json!("v")))
        .collect();
    let more: serde_json::Map<String, Value> = (0..16)
        .map(|index| (format!("b{index}"), json!("v")))
        .collect();
    assert!(report(&mut app, "user:a", &Value::Object(full), &json!({})).is_ok());
    assert!(report(&mut app, "user:b", &Value::Object(more), &json!({})).is_ok());
    assert_eq!(tokens(&app).unwrap().len(), 32);
    let limit = failure(
        "metadata_token_limit",
        "workspace metadata may contain at most 32 tokens",
    );
    let sequence = app.event_hub().current_sequence();
    assert_eq!(
        report(&mut app, "user:c", &json!({"c": "v"}), &json!({"seq": 1})),
        limit
    );
    assert!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .is_empty()
    );
    // A patch that replaces or clears a retained key stays within capacity.
    assert!(
        report(
            &mut app,
            "user:c",
            &json!({"a0": "new", "c": "v", "a1": null}),
            &json!({"seq": 2})
        )
        .is_ok()
    );
    assert_eq!(tokens(&app).unwrap().len(), 32);
    // Sequences: a stale or repeated report changes nothing; a newer one applies.
    for (seq, value, applied) in [(2, "stale", false), (1, "older", false), (3, "newer", true)] {
        assert_eq!(
            report(
                &mut app,
                "user:c",
                &json!({"c": value}),
                &json!({"seq": seq})
            ),
            Ok(ResponseResult::Ok {})
        );
        assert_eq!(tokens(&app).unwrap()["c"] == value, applied, "{seq}");
    }
    assert_eq!(
        app.state.topology.workspaces[0].metadata_token_sequences["user:c"],
        3
    );
    // A stale report is accepted without effect even when it would exceed capacity.
    assert_eq!(
        report(&mut app, "user:c", &json!({"z": "v"}), &json!({"seq": 1})),
        Ok(ResponseResult::Ok {})
    );
    assert!(!tokens(&app).unwrap().contains_key("z"));
    assert_eq!(published(&app, sequence).len(), 2);
    assert_sequence_source_capacity();
}
/// Thirty-two sequenced sources are tracked; a thirty-third new one is refused, while an
/// unsequenced report from it still applies.
fn assert_sequence_source_capacity() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    for index in 0..32 {
        assert!(
            report(
                &mut app,
                &format!("s{index}"),
                &json!({"k": format!("{index}")}),
                &json!({"seq": 1})
            )
            .is_ok()
        );
    }
    let sources = failure(
        "metadata_sequence_source_limit",
        "workspace metadata may track at most 32 sequenced sources",
    );
    assert_eq!(
        report(&mut app, "s32", &json!({"k": "late"}), &json!({"seq": 1})),
        sources
    );
    assert_eq!(tokens(&app).unwrap()["k"], "31");
    assert!(report(&mut app, "s32", &json!({"k": "unsequenced"}), &json!({})).is_ok());
    assert_eq!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .len(),
        32
    );
}

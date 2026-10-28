//! Historical session conversion through the public operation.
#![cfg(test)]
use crate::support;
use maestro_request::presence::Presence;
use maestro_session::parse_session_entries;
use maestro_session::{build_session_context, migrate_session_entries};
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use support::session_files::cases;

#[test]
fn historical_versions_select_only_required_conversions() {
    conversions("historical_versions_select_only_required_conversions");
}

#[test]
fn conversion_rejects_nonnumeric_versions_at_the_consuming_read() {
    conversions("conversion_rejects_nonnumeric_versions_at_the_consuming_read");
}

#[test]
fn conversion_cut_indices_use_final_header_inclusive_ids() {
    conversions("conversion_cut_indices_use_final_header_inclusive_ids");
}

#[test]
fn conversions_preserve_unselected_fields_and_roles() {
    conversions("conversions_preserve_unselected_fields_and_roles");
}

#[test]
fn repeated_conversion_is_inert_after_current_version() {
    let rows = support::session_files::cases("repeated_conversion_is_inert_after_current_version");
    for case in rows {
        assert_eq!(case.input, json!({}));
        let mut entries = vec![
            json!({"type":"session","version":1,"id":"history","timestamp":"2025-01-01T00:00:00Z","cwd":"stored"}),
            json!({"type":"message","id":"old","parentId":null,"timestamp":"2025-01-01T00:00:01Z","message":{"role":"hookMessage","content":"old","timestamp":1}}),
        ];
        migrate_session_entries(&mut entries).unwrap();
        let once = entries.clone();
        migrate_session_entries(&mut entries).unwrap();
        assert_eq!(entries, once);
        normalize(&mut entries);
        assert_eq!(
            json!({"once":entries,"twice":entries}),
            case.expected["ok"],
            "{}",
            case.id
        );
    }
}

#[cfg(all(not(windows), not(target_arch = "wasm32")))]
#[test]
fn converted_context_uses_existing_local_timestamp_policy() {
    if maestro_test_process::selection().is_child() {
        let zone = ["UTC", "America/New_York"]
            .into_iter()
            .find(|zone| maestro_test_process::selection().runs_case(zone))
            .expect("selected timezone");
        let local = if zone == "America/New_York" {
            1_735_711_200_000_i64
        } else {
            1_735_693_200_000_i64
        };
        if zone == "America/New_York" {
            assert_ne!(chrono::Local::now().offset().local_minus_utc(), 0);
        }
        check_converted_timestamps(local);
        return;
    }
    if !maestro_test_process::local_zone_follows_tz() {
        return;
    }
    for zone in ["UTC", "America/New_York"] {
        let output = maestro_test_process::TestProcess::current(
            "converted_context_uses_existing_local_timestamp_policy",
        )
        .unwrap()
        .nocapture()
        .env("TZ", zone)
        .case(zone)
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(windows)]
#[test]
fn converted_context_uses_existing_local_timestamp_policy() {
    use chrono::TimeZone;
    let local = chrono::Local
        .with_ymd_and_hms(2025, 1, 1, 1, 0, 0)
        .earliest()
        .unwrap()
        .timestamp_millis();
    check_converted_timestamps(local);
}

/// Compare converted timestamps against the platform's local-time expectation.
#[cfg(not(target_arch = "wasm32"))]
fn check_converted_timestamps(local: i64) {
    for case in
        support::session_files::cases("converted_context_uses_existing_local_timestamp_policy")
    {
        let mut entries = vec![
            json!({"type":"session","version":1,"id":"history"}),
            json!({"type":"message","message":{"role":"user","content":"a","timestamp":1}}),
            json!({"type":"compaction","firstKeptEntryIndex":1,"summary":"time","tokensBefore":1,"timestamp":case.input["timestamp"]}),
        ];
        migrate_session_entries(&mut entries).unwrap();
        let result = serde_json::to_value(
            build_session_context(&entries, Presence::Missing, None::<&HashMap<_, _>>).unwrap(),
        )
        .unwrap();
        let mut expected = case.expected["ok"].clone();
        if case.input["timestamp"] == "2025-01-01T01:00:00" {
            expected["messages"][0]["timestamp"] = json!(local);
        }
        support::value::assert_value(&result, &expected, &case.id);
    }
}

/// Normalize newly allocated identities only at their owning wire properties.
fn normalize(entries: &mut [Value]) {
    let ids: HashMap<_, _> = entries
        .iter()
        .filter(|entry| entry["type"] != "session")
        .enumerate()
        .map(|(position, entry)| {
            (
                entry["id"].as_str().unwrap().to_owned(),
                format!("{:08}", position + 1),
            )
        })
        .collect();
    for entry in entries {
        if entry["type"] == "session" {
            continue;
        }
        for key in ["id", "parentId", "firstKeptEntryId"] {
            if let Some(id) = entry[key].as_str().and_then(|id| ids.get(id)) {
                entry[key] = id.clone().into();
            }
        }
    }
}
/// Compare the entire converted output and every supplied expected property.
fn conversions(name: &str) {
    for case in cases(name) {
        let mut entries = case
            .input
            .as_array()
            .cloned()
            .unwrap_or_else(|| parse_session_entries(case.input["jsonl"].as_str().unwrap()));
        let version = entries
            .iter()
            .find(|entry| entry["type"] == "session")
            .and_then(|entry| entry["version"].as_f64())
            .unwrap_or(1.0);
        let result = migrate_session_entries(&mut entries);
        if case.expected.get("native_error").is_some() {
            assert_eq!(case.expected["native_error"], "InvalidData");
            assert_eq!(
                result.unwrap_err().kind(),
                std::io::ErrorKind::InvalidData,
                "{}",
                case.id
            );
            continue;
        }
        result.unwrap();
        if case.id == "conversion-wire-cut-1.0" {
            assert_eq!(
                entries[2].as_object().unwrap().keys().collect::<Vec<_>>(),
                [
                    "type",
                    "summary",
                    "tokensBefore",
                    "timestamp",
                    "id",
                    "parentId",
                    "firstKeptEntryId"
                ],
                "{}: converted property order",
                case.id
            );
        }
        if version < 2.0 {
            normalize(&mut entries);
        }
        let mut observed = json!({"entries": entries});
        if case.expected["ok"].get("context").is_some() {
            observed["context"] = serde_json::to_value(
                build_session_context(&entries, Presence::Missing, None::<&HashMap<_, _>>).unwrap(),
            )
            .unwrap();
        }
        support::value::assert_value(&observed, &case.expected["ok"], &case.id);
    }
}

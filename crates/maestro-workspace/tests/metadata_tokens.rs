//! Metadata patch behaviour.
#![cfg(test)]
use maestro_workspace::metadata_tokens::{
    MAX_SEQUENCE_SOURCES, MetadataTokens, SequenceCapacityError, accept_sequence, sequence_is_fresh,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

fn patch(items: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
    items
        .iter()
        .map(|(key, value)| ((*key).into(), value.map(str::to_owned)))
        .collect()
}

#[test]
fn token_patch_replaces_and_deletes_named_keys() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    tokens.patch(
        patch(&[("summary", Some("one")), ("model", Some("large"))]),
        None,
        now,
    );
    tokens.patch(
        patch(&[("summary", Some("two")), ("model", None)]),
        None,
        now,
    );
    assert_eq!(
        tokens.values(),
        HashMap::from([("summary".into(), "two".into())])
    );
}

#[test]
fn token_ttl_only_applies_to_patched_keys() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    tokens.patch(
        patch(&[("short", Some("one"))]),
        Some(Duration::from_secs(1)),
        now,
    );
    tokens.patch(patch(&[("persistent", Some("two"))]), None, now);
    assert!(tokens.expire_at(now + Duration::from_secs(1)));
    assert_eq!(
        tokens.values(),
        HashMap::from([("persistent".into(), "two".into())])
    );
}

#[test]
fn token_values_change_only_on_explicit_expiry() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    tokens.patch(
        patch(&[("summary", Some("temporary"))]),
        Some(Duration::from_secs(1)),
        now,
    );
    assert_eq!(
        tokens.values(),
        HashMap::from([("summary".into(), "temporary".into())])
    );
    assert!(tokens.expire_at(now + Duration::from_secs(1)));
    assert!(tokens.values().is_empty());
}

#[test]
fn late_expiry_removes_all_due_tokens() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    tokens.patch(
        patch(&[("first", Some("one"))]),
        Some(Duration::from_secs(1)),
        now,
    );
    tokens.patch(
        patch(&[("second", Some("two"))]),
        Some(Duration::from_secs(2)),
        now,
    );
    assert!(tokens.expire_at(now + Duration::from_secs(10)));
    assert!(tokens.values().is_empty());
}

#[test]
fn old_deadline_keeps_replacement_token() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    tokens.patch(
        patch(&[("summary", Some("old"))]),
        Some(Duration::from_secs(1)),
        now,
    );
    tokens.patch(
        patch(&[("summary", Some("new"))]),
        Some(Duration::from_secs(5)),
        now,
    );
    assert!(!tokens.expire_at(now + Duration::from_secs(1)));
    assert_eq!(
        tokens.values(),
        HashMap::from([("summary".into(), "new".into())])
    );
}

#[test]
fn persistent_replacement_removes_old_deadline() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    tokens.patch(
        patch(&[("summary", Some("temporary"))]),
        Some(Duration::from_secs(1)),
        now,
    );
    tokens.patch(patch(&[("summary", Some("persistent"))]), None, now);
    assert!(!tokens.expire_at(now + Duration::from_secs(1)));
    assert_eq!(
        tokens.values(),
        HashMap::from([("summary".into(), "persistent".into())])
    );
}

#[test]
fn token_patch_change_and_count_follow_values_and_deadlines() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    let values = patch(&[("empty", Some("")), ("text", Some("value"))]);
    assert_eq!(tokens.key_count_after_patch(&values), 2);
    assert!(tokens.values().is_empty());
    assert!(tokens.patch(values.clone(), None, now));
    assert!(!tokens.patch(values.clone(), None, now));
    assert_eq!(tokens.values().get("empty").map(String::as_str), Some(""));
    assert!(tokens.patch(values, Some(Duration::from_secs(1)), now));
    let deletion = patch(&[("missing", None), ("empty", None)]);
    assert_eq!(tokens.key_count_after_patch(&deletion), 1);
    assert_eq!(tokens.values().len(), 2);
    assert!(!tokens.patch(patch(&[("missing", None)]), None, now));
    assert!(tokens.patch(deletion, None, now));
    assert_eq!(
        tokens.values(),
        HashMap::from([("text".into(), "value".into())])
    );
}

#[test]
fn token_expiry_obeys_boundary_and_checked_deadline() {
    let now = Instant::now();
    let mut tokens = MetadataTokens::default();
    assert_eq!(tokens.next_expiry(), None);
    tokens.patch(
        patch(&[("overflow", Some("persistent"))]),
        Some(Duration::MAX),
        now,
    );
    assert_eq!(tokens.next_expiry(), None);
    tokens.patch(
        patch(&[("timed", Some("one"))]),
        Some(Duration::from_secs(1)),
        now,
    );
    assert_eq!(tokens.next_expiry(), Some(now + Duration::from_secs(1)));
    assert!(!tokens.expire_at(now));
    assert_eq!(tokens.values().len(), 2);
    assert!(tokens.expire_at(now + Duration::from_secs(1)));
    assert!(!tokens.expire_at(now + Duration::from_secs(1)));
    assert_eq!(tokens.next_expiry(), None);
    tokens.patch(patch(&[("zero", Some("now"))]), Some(Duration::ZERO), now);
    assert_eq!(tokens.next_expiry(), Some(now));
    assert!(tokens.expire_at(now));
    assert_eq!(
        tokens.values(),
        HashMap::from([("overflow".into(), "persistent".into())])
    );
}

#[test]
fn sequence_source_limit_keeps_existing_sources_usable() {
    let mut sequences = HashMap::new();
    for index in 0..MAX_SEQUENCE_SOURCES {
        assert_eq!(
            accept_sequence(&mut sequences, &format!("source-{index}"), Some(1)),
            Ok(true)
        );
    }
    assert_eq!(
        accept_sequence(&mut sequences, "one-too-many", Some(1)),
        Err(SequenceCapacityError)
    );
    assert_eq!(
        accept_sequence(&mut sequences, "source-0", Some(1)),
        Ok(false)
    );
    assert_eq!(
        accept_sequence(&mut sequences, "source-0", Some(2)),
        Ok(true)
    );
}

#[test]
fn sequence_admission_covers_absent_stale_and_full_sources() {
    let mut sequences = HashMap::new();
    assert!(sequence_is_fresh(&sequences, "unknown", None));
    assert_eq!(accept_sequence(&mut sequences, "unknown", None), Ok(true));
    assert!(sequences.is_empty());
    for index in 0..MAX_SEQUENCE_SOURCES {
        assert_eq!(
            accept_sequence(&mut sequences, &format!("s{index}"), Some(5)),
            Ok(true)
        );
    }
    let before = sequences.clone();
    for seq in [Some(4), Some(5)] {
        assert!(!sequence_is_fresh(&sequences, "s0", seq));
        assert_eq!(accept_sequence(&mut sequences, "s0", seq), Ok(false));
    }
    assert_eq!(
        accept_sequence(&mut sequences, "extra", Some(1)),
        Err(SequenceCapacityError)
    );
    assert_eq!(accept_sequence(&mut sequences, "extra", None), Ok(true));
    assert_eq!(sequences, before);
    assert!(sequence_is_fresh(&sequences, "s0", Some(6)));
    assert_eq!(accept_sequence(&mut sequences, "s0", Some(6)), Ok(true));
    assert_eq!(sequences["s0"], 6);
}

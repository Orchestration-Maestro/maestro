//! Session event payloads, decisions and owned cancellation through both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::scenario::Driver;

/// Persisted compaction identity and origin fields reach an invoked author handler.
async fn compact(driver: &mut impl Driver) -> Result<(), String> {
    super::session_corpus::run(driver, &["compact-origin"]).await
}
#[test]
fn maestro_compact_event_exposes_persisted_entry() -> Result<(), String> {
    on_both_adapters!(compact)
}

/// Leaf and ancestor identities remain independent of optional stored summaries.
async fn trees(driver: &mut impl Driver) -> Result<(), String> {
    super::session_corpus::run(
        driver,
        &["nullable-pairs", "tree-preparation", "tree-summary-details"],
    )
    .await
}
#[test]
fn maestro_tree_events_keep_nullable_ids_and_summary() -> Result<(), String> {
    on_both_adapters!(trees)
}

/// Cancellation and summary overrides carry decisions without navigation policy.
async fn tree_results(driver: &mut impl Driver) -> Result<(), String> {
    super::session_corpus::run(
        driver,
        &["results", "tree-result-fields", "interacting-tree-results"],
    )
    .await
}
#[test]
fn maestro_tree_results_keep_each_optional_override() -> Result<(), String> {
    on_both_adapters!(tree_results)
}

/// Finite values keep their bits; nonfinite typed edits fail independently of decisions.
async fn numbers(driver: &mut impl Driver) -> Result<(), String> {
    use super::scenario::Decision;
    use serde_json::json;
    super::session_corpus::run(driver, &["numbers", "result-numbers"]).await?;
    let handler = driver.identity("event probe")?;
    for bits in ["7ff0000000000000", "fff0000000000000", "7ff8000000000000"] {
        for field in [
            "tokensBefore",
            "reserveTokens",
            "keepRecentTokens",
            "entryTokens",
        ] {
            let kind = if field == "entryTokens" {
                "session_compact"
            } else {
                "session_before_compact"
            };
            let event = super::session_corpus::base(kind);
            let signal = if kind == "session_before_compact" {
                Some(driver.lend_signal(false)?)
            } else {
                None
            };
            let directive = json!({"numberBits":bits,"numberField":field,"ending":{"fails":"authored failure"}});
            let delivery = driver
                .deliver(handler, &event.to_string(), &directive.to_string(), signal)
                .await?;
            assert_eq!(
                delivery.event,
                Err("extension wrote a non-finite number (Infinity or NaN)".to_owned())
            );
            assert_eq!(
                delivery.decision,
                Decision::Failed("authored failure".to_owned())
            );
        }
        let event = super::session_corpus::base("session_before_compact");
        let signal = driver.lend_signal(false)?;
        let delivery = driver
            .deliver(
                handler,
                &event.to_string(),
                &json!({"resultNumberBits":bits}).to_string(),
                Some(signal),
            )
            .await?;
        assert_eq!(
            delivery.decision,
            Decision::Returned(Err(
                "extension wrote a non-finite number (Infinity or NaN)".to_owned()
            ))
        );
        assert!(delivery.event.is_ok());
    }
    Ok(())
}
#[test]
fn maestro_session_numbers_keep_finite_values_and_reject_nonfinite_edits() -> Result<(), String> {
    on_both_adapters!(numbers)
}

/// Edited fields survive errors; a different event kind does not suppress the decision.
async fn edits(driver: &mut impl Driver) -> Result<(), String> {
    use super::documents::{Ended, ask, replacing, returns};
    use serde_json::json;
    super::session_corpus::run(
        driver,
        &[
            "base",
            "compaction-preparation",
            "settings-enabled",
            "edits",
        ],
    )
    .await?;
    for kind in [
        "session_before_compact",
        "session_compact",
        "session_before_tree",
        "session_tree",
    ] {
        let event = super::session_corpus::base(kind);
        let directive = replacing(
            returns(
                "session_before_tree",
                &json!({"cancel":false,"label":"decision"}),
            ),
            super::documents::document("input"),
        );
        let answer = ask(driver, &event, &directive).await?;
        assert!(answer.event.is_none());
        assert_eq!(
            answer.ended,
            Ended::Returned(Some(json!({"cancel":false,"label":"decision"})))
        );
    }
    Ok(())
}
#[test]
fn maestro_session_edits_survive_errors_and_kind_changes() -> Result<(), String> {
    on_both_adapters!(edits)
}

/// Retained tree signals observe the original owner and all exit paths release resources.
async fn signals(driver: &mut impl Driver) -> Result<(), String> {
    use super::documents::{ask_with, fails, replacing};
    use serde_json::json;
    let event = super::session_corpus::base("session_before_tree");
    refused_signals(driver, &event).await?;
    let before = driver.live();
    unused_tree_signals(driver, before).await?;
    for directive in [
        json!({}),
        fails("authored failure"),
        replacing(json!({}), super::documents::document("input")),
    ] {
        let signal = driver.lend_signal(false)?;
        ask_with(driver, &event, &directive, Some(signal)).await?;
        assert_eq!(driver.live(), before);
    }
    let first = driver.lend_signal(false)?;
    ask_with(
        driver,
        &event,
        &json!({"retain":{"signal":true}}),
        Some(first),
    )
    .await?;
    let second = driver.lend_signal(false)?;
    ask_with(
        driver,
        &event,
        &json!({"retain":{"signal":true}}),
        Some(second),
    )
    .await?;
    driver.abort_signal(first)?;
    ask_with(
        driver,
        &super::documents::document("input"),
        &json!({"report":true}),
        None,
    )
    .await?;
    let observations = driver.transcript();
    let kept = observations
        .iter()
        .rev()
        .find_map(|line| line.strip_prefix("entry kept "))
        .ok_or("missing kept signal report")?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(kept).unwrap(),
        json!([{"cwd":null,"aborted":true},{"cwd":null,"aborted":false}])
    );
    assert!(driver.release("event probe").await);
    assert_eq!(driver.live(), before - 1);
    driver.release_all().await;
    assert_eq!(driver.live(), 0);
    Ok(())
}
#[test]
fn maestro_tree_signals_keep_identity_and_release_owners() -> Result<(), String> {
    on_both_adapters!(signals)
}

/// Refuse absent signals after payload decoding and release the refused invocations.
async fn refused_signals(
    driver: &mut impl Driver,
    event: &serde_json::Value,
) -> Result<(), String> {
    let handler = driver.identity("event probe")?;
    let before = driver.live();
    let entered = driver.transcript();
    assert_eq!(
        driver
            .deliver(handler, &event.to_string(), "{}", None)
            .await
            .unwrap_err(),
        "missing tree signal"
    );
    assert_eq!(driver.transcript(), entered);
    let mut malformed = event.clone();
    malformed["preparation"]
        .as_object_mut()
        .unwrap()
        .shift_remove("targetId");
    assert!(
        driver
            .deliver(handler, &malformed.to_string(), "{}", None)
            .await
            .unwrap_err()
            .contains("targetId")
    );
    let signal = driver.lend_signal(false)?;
    assert!(
        driver
            .deliver(handler, &malformed.to_string(), "{}", Some(signal))
            .await
            .unwrap_err()
            .contains("targetId")
    );
    assert_eq!(driver.live(), before);
    Ok(())
}

/// Notifications consume unused signal resources without exposing them in their JSON.
async fn unused_tree_signals(driver: &mut impl Driver, before: usize) -> Result<(), String> {
    use super::documents::{Answer, ask_with};
    use serde_json::json;
    for kind in ["session_compact", "session_tree"] {
        let event = super::session_corpus::base(kind);
        let signal = driver.lend_signal(false)?;
        assert_eq!(
            ask_with(driver, &event, &json!({}), Some(signal)).await?,
            Answer::returned(&event, None)
        );
        assert_eq!(driver.live(), before);
    }
    Ok(())
}

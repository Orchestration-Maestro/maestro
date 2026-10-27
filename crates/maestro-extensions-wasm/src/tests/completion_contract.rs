//! Authored asynchronous completion contracts run in the component and controlled adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::basic_ui::{install, operate};
use super::scenario::Driver;
use super::ui_host::receipts;

/// Empty candidates, no candidates, optional force and the completed cursor stay distinct.
async fn suggestions(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for (force, decision, expected) in [
        (Value::Null, Value::Null, json!([null, null])),
        (json!(false), json!(""), json!([null, [], "λ/", null])),
        (
            json!(true),
            json!("雪\n"),
            json!([
                null,
                [["雪\n", "snow 雪", ""]],
                "λ/",
                [["λ/", "雪\n"], 1, 0]
            ]),
        ),
    ] {
        operate(driver, json!({ "op": "control", "answer": decision })).await?;
        let from = driver.transcript().len();
        let outcome = operate(
            driver,
            json!({ "op": "completionContract", "force": force }),
        )
        .await?;
        assert_eq!(outcome, json!({ "ok": expected }));
        let seen = receipts(&driver.transcript()[from..]);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].method, "input");
        assert_eq!(seen[1].method, "settle");
        assert_eq!(seen[1].operands, json!([false]));
        let title = json!([["λ/", ""], 1, 0, force]).to_string();
        assert_eq!(seen[0].operands, json!([title, null, false, null]));
    }
    operate(
        driver,
        json!({ "op": "control", "error": "provider failed 雪\nhere" }),
    )
    .await?;
    let outcome = operate(driver, json!({ "op": "completionContract" })).await?;
    assert_eq!(outcome, json!({ "error": "provider failed 雪\nhere" }));
    driver.release_all().await;
    Ok(())
}

#[test]
fn authored_completion_keeps_optional_results_force_and_errors() -> Result<(), String> {
    on_both_adapters!(suggestions)
}

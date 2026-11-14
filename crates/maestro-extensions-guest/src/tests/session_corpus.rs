//! Recorded session dispatch expectations through the author boundary.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::documents::{Answer, Ended, ask};
use super::scenario::Driver;
use serde_json::{Value, json};

/// Decode recorded numbers and ordered sets into the transport's plain JSON values.
pub(super) fn materialize(value: &Value) -> Value {
    match value {
        Value::Object(object) if object.len() == 1 && object.contains_key("f64") => {
            let bits = u64::from_str_radix(object["f64"].as_str().unwrap(), 16).unwrap();
            json!(f64::from_bits(bits))
        }
        Value::Object(object) if object.len() == 1 && object.contains_key("set") => {
            materialize(&object["set"])
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), materialize(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(materialize).collect()),
        value => value.clone(),
    }
}

/// Compare numeric bits as well as record contents and ordered list elements.
pub(super) fn same(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            assert_eq!(a.as_f64().unwrap().to_bits(), b.as_f64().unwrap().to_bits());
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                same(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, b) in b {
                same(&a[key], b);
            }
        }
        _ => assert_eq!(actual, expected),
    }
}

/// Run each retained query of the selected families exactly once per adapter.
pub(super) async fn run(driver: &mut impl Driver, families: &[&str]) -> Result<(), String> {
    let rows: Vec<Value> = serde_json::from_str(include_str!("session_corpus.json")).unwrap();
    for row in rows
        .iter()
        .filter(|row| families.contains(&row["family"].as_str().unwrap()))
    {
        let event = materialize(&row["input"]);
        let result = row.get("result").map(materialize);
        let mut directive = result.as_ref().map_or(json!({}), |result| {
            super::documents::returns(event["type"].as_str().unwrap(), result)
        });
        if let Some(edit) = row.get("edit") {
            directive["sessionEdit"] = edit.clone();
        }
        if let Some(error) = row.get("error") {
            directive["ending"] = json!({"fails":error});
        }
        let Answer {
            event: actual,
            ended,
        } = ask(driver, &event, &directive).await?;
        same(
            &actual.ok_or("missing session event")?,
            &materialize(row.get("after").unwrap_or(&row["input"])),
        );
        check_ending(ended, row.get("error"), result)?;
    }
    Ok(())
}

/// The complete recorded base event of one session kind.
pub(super) fn base(kind: &str) -> Value {
    let rows: Vec<Value> = serde_json::from_str(include_str!("session_corpus.json")).unwrap();
    materialize(
        &rows
            .iter()
            .find(|row| row["family"] == "base" && row["input"]["type"] == kind)
            .unwrap()["input"],
    )
}

/// Compare the independently returned decision or authored error.
fn check_ending(ended: Ended, error: Option<&Value>, result: Option<Value>) -> Result<(), String> {
    if let Some(error) = error {
        assert_eq!(ended, Ended::Failed(error.as_str().unwrap().to_owned()));
        return Ok(());
    }
    match ended {
        Ended::Returned(actual) => {
            assert_eq!(actual.is_some(), result.is_some());
            if let (Some(actual), Some(expected)) = (actual, result) {
                same(&actual, &expected);
            }
        }
        Ended::Failed(error) => return Err(error),
    }
    Ok(())
}

/// The eight recorded message families, for prepared-message consumer coverage.
pub(super) fn messages() -> Value {
    let rows: Vec<Value> = serde_json::from_str(include_str!("session_corpus.json")).unwrap();
    Value::Array(
        rows.iter()
            .filter(|row| row["family"] == "nested-messages")
            .map(|row| materialize(&row["input"]["branchEntries"][0]["message"]))
            .collect(),
    )
}

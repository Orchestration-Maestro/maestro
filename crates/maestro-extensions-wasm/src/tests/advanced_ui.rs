//! Editor and theme requests of the retained UI on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::basic_ui::{bind, install, operate, retained_call, retained_owner};
use super::scenario::Driver;
use super::session_corpus::same;
use super::ui_host::receipts;

/// Editor texts covering empty, Unicode, both line endings and styling bytes.
const TEXTS: [&str; 5] = [
    "",
    "literal 雪 \u{feff}",
    "first\nsecond\r\nthird",
    "\u{1b}[7mstyled\u{1b}[0m  ",
    "  padded  ",
];

/// Runs one request through a fresh acquisition, checks the one host request it made and
/// returns the author's answer.
async fn acquired(
    driver: &mut impl Driver,
    call: &Value,
    method: &str,
    operands: &Value,
) -> Result<Value, String> {
    let from = driver.transcript().len();
    let answer = operate(driver, json!({ "op": "acquire", "call": call })).await?;
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 1, "exactly one host request for {call}");
    assert_eq!(seen[0].method, method, "method of {call}");
    same(&seen[0].operands, operands);
    Ok(answer)
}

/// Each text crosses unchanged in both directions, and a paste stays a paste.
async fn literal_texts(driver: &mut impl Driver) -> Result<(), String> {
    for text in TEXTS {
        let set = json!({ "op": "setEditorText", "text": text });
        let answer = acquired(driver, &set, "setEditorText", &json!([text])).await?;
        assert_eq!(answer, json!({ "ok": true }), "accepted replacement");
        let get = json!({ "op": "getEditorText" });
        let answer = acquired(driver, &get, "getEditorText", &json!([])).await?;
        assert_eq!(answer, json!({ "ok": text }), "the host's text is returned");
        let paste = json!({ "op": "pasteToEditor", "text": text });
        let answer = acquired(driver, &paste, "pasteToEditor", &json!([text])).await?;
        assert_eq!(answer, json!({ "ok": true }), "accepted paste");
    }
    Ok(())
}

/// Two owners acquired under different bindings each read the text of their own binding.
async fn selected_owners(driver: &mut impl Driver) -> Result<(), String> {
    retained_owner(driver, "retain").await?;
    bind(driver, 2).await?;
    retained_owner(driver, "retain").await?;
    for (index, text) in [(0, "first owner"), (1, "second owner")] {
        let set = json!({ "op": "setEditorText", "text": text });
        assert_eq!(
            retained_call(driver, index, &set).await?,
            json!({ "ok": "done" })
        );
    }
    let from = driver.transcript().len();
    for (index, text) in [(0, "first owner"), (1, "second owner")] {
        let get = json!({ "op": "getEditorText" });
        assert_eq!(
            retained_call(driver, index, &get).await?,
            json!({ "ok": text }),
            "owner {index} reads its own binding"
        );
    }
    let keys: Vec<usize> = receipts(&driver.transcript()[from..])
        .iter()
        .map(|seen| seen.key)
        .collect();
    assert_eq!(keys, [0, 2], "each read reached the binding of its owner");
    Ok(())
}

/// A rejected editor request returns the host's message and leaves the owner usable.
async fn editor_rejections(driver: &mut impl Driver) -> Result<(), String> {
    let calls = [
        json!({ "op": "pasteToEditor", "text": "x" }),
        json!({ "op": "setEditorText", "text": "x" }),
        json!({ "op": "getEditorText" }),
    ];
    for (call, error) in calls.iter().zip(["", "editor 雪\nunavailable", "closed"]) {
        let operation = json!({ "op": "control", "error": error });
        assert_eq!(operate(driver, operation).await?, json!("controlled"));
        assert_eq!(
            retained_call(driver, 0, call).await?,
            json!({ "error": error }),
            "exact host rejection of {call}"
        );
    }
    let get = json!({ "op": "getEditorText" });
    assert_eq!(
        retained_call(driver, 0, &get).await?,
        json!({ "ok": "first owner" }),
        "the owner still reads its text after the rejections"
    );
    Ok(())
}

/// Paste, replacement and reading reach the editor of the owner that made them.
async fn editor_text(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    literal_texts(driver).await?;
    selected_owners(driver).await?;
    editor_rejections(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI resource stays lent");
    Ok(())
}

#[test]
fn editor_text_operations_reach_the_active_editor() -> Result<(), String> {
    on_both_adapters!(editor_text)
}

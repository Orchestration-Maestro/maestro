//! Editor and theme requests of the retained UI on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::basic_ui::{answer, bind, install, operate, retained_call, retained_owner};
use super::scenario::Driver;
use super::session_corpus::same;
use super::theme_host::{Reply, corpus};
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

/// Installs the theme route and retains one theme the host lends under each key, in order.
async fn retain_themes(driver: &mut impl Driver, keys: &[&str]) -> Result<(), String> {
    driver.run_plain_command("capture", "theme").await?;
    let handler = driver.identity("renderer theme")?;
    let message = super::renderer_callbacks::message();
    for key in keys {
        let returned = driver.renderer(handler, &message, false, key).await?;
        assert!(
            returned.is_none(),
            "the theme renderer returns no component"
        );
    }
    Ok(())
}

/// Calls one method on the retained theme `index` and returns the author's answer.
async fn themed(
    driver: &mut impl Driver,
    index: usize,
    method: &str,
    operands: &Value,
) -> Result<Value, String> {
    let request = json!({ "index": index, "method": method, "args": operands });
    driver
        .run_plain_command("theme", &request.to_string())
        .await?;
    answer(driver, "entry theme-answer ")
}

/// Calls one method on the retained theme `index` and checks the one request the host received.
async fn reached(
    driver: &mut impl Driver,
    index: usize,
    key: &str,
    method: &str,
    operands: &Value,
) -> Result<Value, String> {
    let from = driver.transcript().len();
    let answer = themed(driver, index, method, operands).await?;
    let seen = super::theme_host::receipts(&driver.transcript()[from..]);
    assert_eq!(
        seen.len(),
        1,
        "exactly one host request for {method} {operands}"
    );
    assert_eq!(seen[0].key, key, "theme of {method} {operands}");
    assert_eq!(seen[0].method, method, "method of {method} {operands}");
    same(&seen[0].operands, operands);
    Ok(answer)
}

/// Releases the retained themes and every registration.
async fn release_themes(driver: &mut impl Driver) -> Result<(), String> {
    driver.run_plain_command("theme", "release").await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no theme resource stays lent");
    Ok(())
}

/// The recorded replies of the dark theme whose method is one of `methods`.
fn dark_replies(methods: &[&str]) -> Vec<Reply> {
    let mut palettes = corpus().palettes;
    let dark = palettes.remove("dark").unwrap();
    dark.replies
        .into_iter()
        .filter(|reply| methods.contains(&reply.method.as_str()))
        .collect()
}

/// Every style answer is the host's own bytes, including each style's embedded close.
async fn style_bytes(driver: &mut impl Driver) -> Result<(), String> {
    let styles = [
        "fg",
        "bg",
        "bold",
        "italic",
        "underline",
        "inverse",
        "strikethrough",
    ];
    let mut rows = dark_replies(&styles);
    rows.retain(|row| row.answer.get("ok").is_some());
    assert_eq!(rows.len(), 56, "recorded style rows");
    retain_themes(driver, &["dark"]).await?;
    for row in &rows {
        let answer = reached(driver, 0, "dark", &row.method, &row.operands).await?;
        assert_eq!(answer, row.answer, "{} {}", row.method, row.operands);
    }
    release_themes(driver).await
}

#[test]
fn retained_theme_preserves_style_bytes() -> Result<(), String> {
    on_both_adapters!(style_bytes)
}

/// Unknown colors keep the host's two messages, and known colors answer beside them.
async fn color_errors(driver: &mut impl Driver) -> Result<(), String> {
    let rows = dark_replies(&["fg", "bg", "getFgAnsi", "getBgAnsi"]);
    retain_themes(driver, &["dark"]).await?;
    let mut errors = Vec::new();
    for row in &rows {
        let answer = reached(driver, 0, "dark", &row.method, &row.operands).await?;
        assert_eq!(answer, row.answer, "{} {}", row.method, row.operands);
        if let Some(error) = answer["error"].as_str() {
            errors.push((row.method.clone(), error.to_owned()));
        }
    }
    let expected = [
        ("fg", "Unknown theme color: missing"),
        ("bg", "Unknown theme background color: missing"),
        ("getFgAnsi", "Unknown theme color: missing"),
        ("getBgAnsi", "Unknown theme background color: missing"),
    ]
    .map(|(method, error)| (method.to_owned(), error.to_owned()));
    assert_eq!(errors, expected, "each unknown color keeps its own message");
    assert_eq!(
        themed(driver, 0, "getFgAnsi", &json!(["accent"])).await?,
        json!({ "ok": "\u{1b}[38;2;138;190;183m" }),
        "a known color still answers after the errors"
    );
    release_themes(driver).await
}

#[test]
fn theme_colors_keep_unknown_key_errors() -> Result<(), String> {
    on_both_adapters!(color_errors)
}

/// The color requests that observe a palette.
fn color_requests() -> [(&'static str, Value); 4] {
    [
        ("fg", json!(["accent", "x"])),
        ("bg", json!(["selectedBg", "x"])),
        ("getFgAnsi", json!(["accent"])),
        ("getBgAnsi", json!(["selectedBg"])),
    ]
}

/// The observations of one theme state, read through the retained theme `index`.
async fn observe(driver: &mut impl Driver, index: usize) -> Result<Value, String> {
    let mut observed = serde_json::Map::new();
    let metadata = ["name", "sourcePath", "sourceInfo", "getColorMode"].map(|m| (m, json!([])));
    for (method, operands) in metadata.into_iter().chain(color_requests()) {
        let answer = themed(driver, index, method, &operands).await?;
        observed.insert(method.to_owned(), answer["ok"].clone());
    }
    Ok(Value::Object(observed))
}

/// The observations a recorded palette answers.
fn recorded(palette: &str) -> Value {
    let corpus = corpus();
    let state = &corpus.palettes[palette];
    let mut recorded = json!({
        "name": state.name,
        "sourcePath": state.source_path,
        "sourceInfo": state.source_info,
        "getColorMode": state.mode,
    });
    for (method, operands) in color_requests() {
        recorded[method] = state
            .replies
            .iter()
            .find(|reply| reply.method == method && reply.operands == operands)
            .map(|reply| reply.answer["ok"].clone())
            .unwrap_or_default();
    }
    recorded
}

/// Both aliases of the live theme observe a publication, while a separately lent theme keeps
/// its own state.
async fn publication(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        observe(driver, 0).await?,
        recorded("dark"),
        "before publication"
    );
    assert_eq!(
        observe(driver, 2).await?,
        recorded("quantized"),
        "the other theme"
    );
    let publish = json!({ "theme": "dark", "palette": "alternate" });
    driver
        .run_plain_command("theme", &format!("publish {publish}"))
        .await?;
    for index in [0, 1] {
        assert_eq!(
            observe(driver, index).await?,
            recorded("alternate"),
            "retained theme {index} after publication"
        );
    }
    assert_eq!(
        observe(driver, 2).await?,
        recorded("quantized"),
        "unpublished theme"
    );
    Ok(())
}

/// A provenance edit through one alias crosses as its complete document and is read through
/// both, and clearing it reads as absent.
async fn provenance(driver: &mut impl Driver) -> Result<(), String> {
    let edited = corpus().edited;
    let from = driver.transcript().len();
    assert_eq!(
        themed(driver, 0, "setSourceInfo", &json!([edited])).await?,
        json!({ "ok": null })
    );
    let seen = super::theme_host::receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 1, "one provenance request");
    let document = seen[0].operands[0].as_str().unwrap_or_default();
    assert_eq!(
        serde_json::from_str::<Value>(document).map_err(|error| error.to_string())?,
        edited,
        "the host receives the complete provenance document"
    );
    for index in [0, 1] {
        assert_eq!(
            themed(driver, index, "sourceInfo", &json!([])).await?,
            json!({ "ok": edited }),
            "retained theme {index} reads the edited provenance"
        );
    }
    assert_eq!(
        themed(driver, 1, "setSourceInfo", &json!([null])).await?,
        json!({ "ok": null })
    );
    assert_eq!(
        themed(driver, 0, "sourceInfo", &json!([])).await?,
        json!({ "ok": null }),
        "a cleared provenance reads as absent"
    );
    Ok(())
}

/// Retained aliases of the live theme observe a publication and a provenance edit.
async fn replacement(driver: &mut impl Driver) -> Result<(), String> {
    retain_themes(driver, &["dark", "dark", "quantized"]).await?;
    publication(driver).await?;
    provenance(driver).await?;
    release_themes(driver).await
}

#[test]
fn retained_theme_observes_palette_and_metadata_replacement() -> Result<(), String> {
    on_both_adapters!(replacement)
}

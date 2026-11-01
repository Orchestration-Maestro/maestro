//! Retained UI capabilities on both adapters.
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

/// Supplies the host's next dialog decision, runs one dialog through a fresh acquisition,
/// checks the one host request it made and returns the author's answer.
async fn asked(
    driver: &mut impl Driver,
    call: &Value,
    decision: &Value,
    receipt: (&str, &Value),
) -> Result<Value, String> {
    let control = json!({ "op": "control", "answer": decision });
    assert_eq!(operate(driver, control).await?, json!("controlled"));
    let from = driver.transcript().len();
    let answer = operate(driver, json!({ "op": "dialog", "call": call })).await?;
    let seen = receipts(&driver.transcript()[from..]);
    assert_eq!(seen.len(), 1, "exactly one host request for {call}");
    assert_eq!(seen[0].method, receipt.0, "method of {call}");
    same(&seen[0].operands, receipt.1);
    Ok(answer)
}

/// Each selector choice list keeps its order, repetitions, empty entries and literal text,
/// and an empty choice stays distinct from a dismissal.
async fn selections(driver: &mut impl Driver) -> Result<(), String> {
    let rows = [
        (
            "Pick",
            json!(["a", "b", "a"]),
            json!("b"),
            json!({ "ok": "b" }),
        ),
        ("", json!([]), Value::Null, json!({ "ok": null })),
        (
            "選択 雪",
            json!(["雪 \u{feff}", "", "line\r\nnext"]),
            json!(""),
            json!({ "ok": "" }),
        ),
    ];
    for (title, options, decision, expected) in rows {
        let call = json!({ "op": "select", "title": title, "options": options });
        let operands = json!([title, options, null, null]);
        let answer = asked(driver, &call, &decision, ("select", &operands)).await?;
        assert_eq!(answer, expected, "selector {title:?} answered {decision}");
    }
    Ok(())
}

/// A confirmation keeps an empty message and resolves a dismissal to false.
async fn confirmations(driver: &mut impl Driver) -> Result<(), String> {
    for (message, decision, expected) in [
        ("", json!(true), true),
        ("Proceed?\nnow 雪", json!(false), false),
        ("Proceed?", Value::Null, false),
    ] {
        let call = json!({ "op": "confirm", "title": "Sure", "message": message });
        let operands = json!(["Sure", message, null, null]);
        let answer = asked(driver, &call, &decision, ("confirm", &operands)).await?;
        assert_eq!(answer, json!({ "ok": expected }), "confirmation {decision}");
    }
    Ok(())
}

/// An absent placeholder or prefill stays distinct from an empty one, and each answer or
/// dismissal returns unchanged.
async fn texts(driver: &mut impl Driver) -> Result<(), String> {
    let rows = [
        (json!(null), json!("typed 雪"), json!({ "ok": "typed 雪" })),
        (json!(""), Value::Null, json!({ "ok": null })),
        (json!("a\r\nb"), json!(""), json!({ "ok": "" })),
    ];
    for (supplied, decision, expected) in rows {
        let call = json!({ "op": "input", "title": "Name", "placeholder": supplied });
        let operands = json!(["Name", supplied, null, null]);
        let answer = asked(driver, &call, &decision, ("input", &operands)).await?;
        assert_eq!(answer, expected, "input with placeholder {supplied}");
        let call = json!({ "op": "editor", "title": "Edit", "prefill": supplied });
        let operands = json!(["Edit", supplied]);
        let answer = asked(driver, &call, &decision, ("editor", &operands)).await?;
        assert_eq!(answer, expected, "editor with prefill {supplied}");
    }
    Ok(())
}

/// The literal inputs of every dialog reach the host, and its answer, dismissal or rejection
/// returns unchanged.
async fn dialog_inputs(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    selections(driver).await?;
    confirmations(driver).await?;
    texts(driver).await?;
    let control = json!({ "op": "control", "error": "dialog 雪\nrefused" });
    assert_eq!(operate(driver, control).await?, json!("controlled"));
    let call = json!({ "op": "select", "title": "Pick", "options": ["a"] });
    assert_eq!(
        operate(driver, json!({ "op": "dialog", "call": call })).await?,
        json!({ "error": "dialog 雪\nrefused" }),
        "a rejection is not a dismissal"
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI resource stays lent");
    Ok(())
}

#[test]
fn dialog_requests_preserve_owned_inputs() -> Result<(), String> {
    on_both_adapters!(dialog_inputs)
}

/// Supplies the host's next dialog decision, runs one dialog through a fresh acquisition,
/// holding the host's answer pending when `held`, and returns the author's answer with the
/// host's receipts as method and operands.
async fn signaled(
    driver: &mut impl Driver,
    call: &Value,
    decision: &Value,
    held: bool,
) -> Result<(Value, Vec<(String, Value)>), String> {
    let control = json!({ "op": "control", "answer": decision });
    assert_eq!(operate(driver, control).await?, json!("controlled"));
    let from = driver.transcript().len();
    let operation = json!({ "op": "dialog", "call": call }).to_string();
    if held {
        driver.command_held("ui", &operation).await?;
    } else {
        driver.run_plain_command("ui", &operation).await?;
    }
    let seen = receipts(&driver.transcript()[from..])
        .into_iter()
        .map(|seen| (seen.method, seen.operands))
        .collect();
    Ok((answer(driver, "entry ui-answer ")?, seen))
}

/// The receipt of one dialog, whose operands carry the signal it received, and the signal
/// state the host read when the dialog settled.
fn signal_receipts(method: &str, operands: Value, settled: bool) -> Vec<(String, Value)> {
    vec![
        (method.to_owned(), operands),
        ("settle".to_owned(), json!([settled])),
    ]
}

/// An abort before the call and an abort while the dialog waits both reach its host
/// signal, while the host still decides the answer.
async fn signal_aborts(driver: &mut impl Driver) -> Result<(), String> {
    let aborted = json!({ "op": "select", "title": "Pick", "options": ["a"],
        "opts": { "signal": "aborted" } });
    let (answer, seen) = signaled(driver, &aborted, &json!("a"), false).await?;
    assert_eq!(
        answer,
        json!({ "ok": "a" }),
        "the host still decides an aborted dialog"
    );
    assert_eq!(
        seen,
        signal_receipts("select", json!(["Pick", ["a"], true, null]), true)
    );
    let live = json!({ "op": "confirm", "title": "Sure", "message": "",
        "opts": { "signal": "live" } });
    let (answer, seen) = signaled(driver, &live, &json!(true), false).await?;
    assert_eq!(answer, json!({ "ok": true }));
    assert_eq!(
        seen,
        signal_receipts("confirm", json!(["Sure", "", false, null]), false)
    );
    let pending = json!({ "op": "input", "title": "Name",
        "opts": { "signal": "abortPending" } });
    let (answer, seen) = signaled(driver, &pending, &Value::Null, true).await?;
    assert_eq!(
        answer,
        json!({ "ok": null }),
        "an abort while waiting reaches the host"
    );
    assert_eq!(
        seen,
        signal_receipts("input", json!(["Name", null, false, null]), true)
    );
    Ok(())
}

/// A kept caller signal serves a later dialog through its alias, holds no listener once its
/// dialogs returned, and a later abort reaches only the dialog that follows it.
async fn kept_signal(driver: &mut impl Driver) -> Result<(), String> {
    let alias = json!({ "op": "select", "title": "Again", "options": [],
        "opts": { "signal": { "kept": 1 } } });
    let (_, seen) = signaled(driver, &alias, &Value::Null, false).await?;
    assert_eq!(
        seen,
        signal_receipts("select", json!(["Again", [], false, null]), false)
    );
    let from = driver.transcript().len();
    let abort = json!({ "op": "abortKept", "index": 1 });
    assert_eq!(
        operate(driver, abort).await?,
        json!({ "ok": { "listeners": 0 } }),
        "both returned dialogs removed their listeners from the kept signal"
    );
    assert!(
        receipts(&driver.transcript()[from..]).is_empty(),
        "an abort after the dialogs returned reaches no host dialog"
    );
    let (_, seen) = signaled(driver, &alias, &Value::Null, false).await?;
    assert_eq!(
        seen,
        signal_receipts("select", json!(["Again", [], true, null]), true)
    );
    Ok(())
}

/// Each dialog follows its caller signal with its own host signal, and the caller's listener
/// is removed once the dialog returns.
async fn signal_retention(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    signal_aborts(driver).await?;
    kept_signal(driver).await?;
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "every host signal was released");
    Ok(())
}

#[test]
fn dialog_signal_is_retained_after_return() -> Result<(), String> {
    on_both_adapters!(signal_retention)
}

/// Every timeout reaches the selected dialog as the supplied milliseconds, and a signal and
/// a timeout supplied apart reach only the dialog they were supplied to.
async fn timeouts(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for timeout in [
        json!(null),
        json!(0.0),
        json!(-1.0),
        json!(2.5),
        json!(1500.0),
    ] {
        let opts = json!({ "timeout": timeout });
        let call = json!({ "op": "select", "title": "T", "options": ["a"], "opts": opts });
        let operands = json!(["T", ["a"], null, timeout]);
        asked(driver, &call, &json!("a"), ("select", &operands)).await?;
        let call = json!({ "op": "confirm", "title": "T", "message": "m", "opts": opts });
        let operands = json!(["T", "m", null, timeout]);
        asked(driver, &call, &json!(true), ("confirm", &operands)).await?;
        let call = json!({ "op": "input", "title": "T", "opts": opts });
        let operands = json!(["T", null, null, timeout]);
        asked(driver, &call, &json!("x"), ("input", &operands)).await?;
    }
    let signal_only = json!({ "op": "select", "title": "S", "options": [],
        "opts": { "signal": "live" } });
    let (_, seen) = signaled(driver, &signal_only, &Value::Null, false).await?;
    assert_eq!(
        seen,
        signal_receipts("select", json!(["S", [], false, null]), false)
    );
    let timeout_only = json!({ "op": "confirm", "title": "C", "message": "",
        "opts": { "timeout": 2.5 } });
    let operands = json!(["C", "", null, 2.5]);
    asked(driver, &timeout_only, &json!(false), ("confirm", &operands)).await?;
    let both = json!({ "op": "input", "title": "I",
        "opts": { "signal": { "kept": 0 }, "timeout": 0.25 } });
    let (_, seen) = signaled(driver, &both, &Value::Null, false).await?;
    assert_eq!(
        seen,
        signal_receipts("input", json!(["I", null, false, 0.25]), false)
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI resource stays lent");
    Ok(())
}

#[test]
fn dialog_timeout_is_forwarded_without_guest_policy() -> Result<(), String> {
    on_both_adapters!(timeouts)
}

/// Runs one dialog through the context of `route` and returns the author's answer with the
/// acquisition keys the host's receipts name.
async fn routed_dialog(
    driver: &mut impl Driver,
    route: &str,
    call: &Value,
) -> Result<(Value, Vec<usize>), String> {
    let from = driver.transcript().len();
    let operation = json!({ "op": "dialog", "call": call }).to_string();
    if route == "event" {
        driver.deliver_input("event ui", &operation).await?;
    } else {
        driver.run_plain_command("ui", &operation).await?;
    }
    let keys = receipts(&driver.transcript()[from..])
        .iter()
        .map(|seen| seen.key)
        .collect();
    Ok((answer(driver, "entry ui-answer ")?, keys))
}

/// Event and command contexts each acquire the UI of the binding current at acquisition.
async fn dialog_owners(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let call = json!({ "op": "select", "title": "Pick", "options": ["a", "b"] });
    for (binding, route, decision) in [(1, "event", "a"), (2, "command", "b"), (3, "event", "b")] {
        let control = json!({ "op": "control", "binding": binding, "answer": decision });
        assert_eq!(operate(driver, control).await?, json!("controlled"));
        let (answer, keys) = routed_dialog(driver, route, &call).await?;
        assert_eq!(answer, json!({ "ok": decision }), "{route} dialog answer");
        assert_eq!(
            keys,
            [binding],
            "the {route} dialog reached binding {binding}"
        );
    }
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI resource stays lent");
    Ok(())
}

#[test]
fn each_context_acquires_the_selected_ui_owner() -> Result<(), String> {
    on_both_adapters!(dialog_owners)
}

/// Each dialog returns the host's empty, Unicode or multi-line rejection unchanged, never as a
/// dismissal, and the same UI keeps answering afterwards.
async fn dialog_rejections(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let calls = [
        json!({ "op": "select", "title": "Pick", "options": ["a"] }),
        json!({ "op": "confirm", "title": "Sure", "message": "m" }),
        json!({ "op": "input", "title": "Name", "placeholder": "" }),
        json!({ "op": "editor", "title": "Edit" }),
    ];
    for call in &calls {
        for error in ["", "dialog 雪 \u{feff}", "first\nsecond\r\nthird"] {
            let control = json!({ "op": "control", "error": error, "answer": "kept" });
            assert_eq!(operate(driver, control).await?, json!("controlled"));
            assert_eq!(
                operate(driver, json!({ "op": "dialog", "call": call })).await?,
                json!({ "error": error }),
                "exact host rejection of {call}"
            );
        }
    }
    let control = json!({ "op": "control", "answer": "after" });
    assert_eq!(operate(driver, control).await?, json!("controlled"));
    assert_eq!(
        operate(driver, json!({ "op": "dialog", "call": calls[0] })).await?,
        json!({ "ok": "after" }),
        "the UI still answers after its rejections"
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI resource stays lent");
    Ok(())
}

#[test]
fn advanced_ui_rejections_keep_exact_messages() -> Result<(), String> {
    on_both_adapters!(dialog_rejections)
}

/// One listener's decision as the driver reports it.
fn decided(consume: Option<bool>, data: Option<&str>) -> Value {
    json!({ "ok": { "consume": consume, "data": data } })
}

/// Each listener answers every chunk with its own decision: no result, absent members, empty
/// or replaced data, consumption either way, both together, or its exact failure.
async fn terminal_decisions(driver: &mut impl Driver) -> Result<(), String> {
    for label in ["a", "b"] {
        let listen = json!({ "op": "listen", "label": label });
        assert_eq!(operate(driver, listen).await?, json!({ "ok": "listening" }));
    }
    let rows = [
        ("none", json!({ "ok": null }), json!({ "ok": null })),
        ("absent", decided(None, None), decided(None, None)),
        ("empty", decided(None, Some("")), decided(None, Some(""))),
        (
            "雪 \u{1b}[A",
            decided(None, Some("a 雪 \u{1b}[A")),
            decided(None, Some("b 雪 \u{1b}[A")),
        ),
        (
            "pass",
            decided(Some(false), None),
            decided(Some(false), None),
        ),
        (
            "consume",
            decided(Some(true), None),
            decided(Some(true), None),
        ),
        (
            "both",
            decided(Some(true), Some("a both")),
            decided(Some(true), Some("b both")),
        ),
        (
            "fail",
            json!({ "error": "a failed 雪\nhere" }),
            json!({ "error": "b failed 雪\nhere" }),
        ),
    ];
    for (data, first, second) in rows {
        assert_eq!(
            driver.terminal_input(data).await?,
            [first, second],
            "listeners in registration order answer {data:?}"
        );
    }
    Ok(())
}

/// An unsubscribed listener stops answering, a dropped subscription keeps its listener, and a
/// later listener answers after the ones still live.
async fn terminal_removal(driver: &mut impl Driver) -> Result<(), String> {
    let unlisten = json!({ "op": "unlisten", "label": "a" });
    assert_eq!(
        operate(driver, unlisten).await?,
        json!({ "ok": "unsubscribed" })
    );
    assert_eq!(
        driver.terminal_input("x").await?,
        [decided(None, Some("b x"))]
    );
    let forget = json!({ "op": "forget", "label": "b" });
    assert_eq!(operate(driver, forget).await?, json!({ "ok": "dropped" }));
    assert_eq!(
        driver.terminal_input("x").await?,
        [decided(None, Some("b x"))],
        "dropping the subscription keeps the listener"
    );
    let listen = json!({ "op": "listen", "label": "c" });
    assert_eq!(operate(driver, listen).await?, json!({ "ok": "listening" }));
    assert_eq!(
        driver.terminal_input("y").await?,
        [decided(None, Some("b y")), decided(None, Some("c y"))]
    );
    Ok(())
}

/// Terminal input listeners keep their decisions, failures and removal through both adapters.
async fn terminal_input(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    terminal_decisions(driver).await?;
    terminal_removal(driver).await?;
    let emitted: Vec<Value> = driver
        .bus_emissions()
        .into_iter()
        .filter(|(channel, _)| channel == "terminal-input")
        .map(|(_, data)| data)
        .collect();
    assert_eq!(
        emitted.first(),
        Some(&json!(["a", "none"])),
        "listeners emit through their call"
    );
    assert_eq!(emitted.len(), 20, "every invocation emitted once");
    driver.release_all().await;
    Ok(())
}

#[test]
fn terminal_input_keeps_transform_consume_and_removal() -> Result<(), String> {
    on_both_adapters!(terminal_input)
}

/// Runs one theme or tool expansion request through a fresh acquisition and returns the
/// author's answer with the host's receipts as method and operands.
async fn theme_request(
    driver: &mut impl Driver,
    call: &Value,
) -> Result<(Value, Vec<(String, Value)>), String> {
    let from = driver.transcript().len();
    let answer = operate(driver, json!({ "op": "theme", "call": call })).await?;
    let seen = receipts(&driver.transcript()[from..])
        .into_iter()
        .map(|seen| (seen.method, seen.operands))
        .collect();
    Ok((answer, seen))
}

/// The answer of one theme request, which must reach the host as `method` with `operand`.
async fn ui_themed(
    driver: &mut impl Driver,
    call: Value,
    method: &str,
    operand: Value,
) -> Result<Value, String> {
    let (answer, seen) = theme_request(driver, &call).await?;
    assert_eq!(
        seen.first(),
        Some(&(method.to_owned(), json!([operand]))),
        "{call}"
    );
    Ok(answer)
}

/// Listing keeps the host's order and absent paths, and loading reads a theme without
/// selecting it.
async fn theme_lookup(driver: &mut impl Driver) -> Result<(), String> {
    let current = json!({ "op": "current" });
    assert_eq!(
        ui_themed(driver, current.clone(), "theme", Value::Null).await?,
        json!({ "ok": "dark" })
    );
    let all = ui_themed(
        driver,
        json!({ "op": "getAllThemes" }),
        "getAllThemes",
        Value::Null,
    )
    .await?;
    assert_eq!(
        all,
        json!({ "ok": [
            { "name": "alternate", "path": "/authored/theme.json" },
            { "name": "dark", "path": "/themes/dark.json" },
            { "name": "plain", "path": null },
        ] })
    );
    let load = json!({ "op": "getTheme", "name": "alternate" });
    assert_eq!(
        ui_themed(driver, load, "getTheme", json!("alternate")).await?,
        json!({ "ok": "alternate" })
    );
    let missing = json!({ "op": "getTheme", "name": "missing" });
    assert_eq!(
        ui_themed(driver, missing, "getTheme", json!("missing")).await?,
        json!({ "ok": null })
    );
    assert_eq!(
        ui_themed(driver, current, "theme", Value::Null).await?,
        json!({ "ok": "dark" }),
        "loading did not select"
    );
    Ok(())
}

/// Selecting by name and selecting a retained theme reach the host as distinct requests, each
/// with its own result.
async fn theme_selection(driver: &mut impl Driver) -> Result<(), String> {
    let current = json!({ "op": "current" });
    let by_name = json!({ "op": "setThemeName", "name": "alternate" });
    assert_eq!(
        ui_themed(driver, by_name, "setThemeName", json!("alternate")).await?,
        json!({ "ok": { "success": true, "error": null } })
    );
    assert_eq!(
        ui_themed(driver, current.clone(), "theme", Value::Null).await?,
        json!({ "ok": "alternate" })
    );
    let missing = json!({ "op": "setThemeName", "name": "missing" });
    assert_eq!(
        ui_themed(driver, missing, "setThemeName", json!("missing")).await?,
        json!({ "ok": { "success": false, "error": "Theme not found: missing" } })
    );
    let (answer, seen) =
        theme_request(driver, &json!({ "op": "setLoaded", "name": "quantized" })).await?;
    assert_eq!(answer, json!({ "ok": { "success": true, "error": null } }));
    let methods: Vec<_> = seen
        .iter()
        .map(|(method, operands)| (method.as_str(), operands.clone()))
        .collect();
    assert_eq!(
        methods,
        [
            ("getTheme", json!(["quantized"])),
            ("setTheme", json!(["quantized"]))
        ]
    );
    assert_eq!(
        ui_themed(driver, current, "theme", Value::Null).await?,
        json!({ "ok": "dark" }),
        "the quantized palette is named dark"
    );
    Ok(())
}

/// Tool expansion changes both ways through the host.
async fn tool_expansion(driver: &mut impl Driver) -> Result<(), String> {
    for expanded in [true, false] {
        let get = json!({ "op": "getToolsExpanded" });
        assert_eq!(
            ui_themed(driver, get.clone(), "getToolsExpanded", Value::Null).await?,
            json!({ "ok": !expanded })
        );
        let set = json!({ "op": "setToolsExpanded", "expanded": expanded });
        assert_eq!(
            ui_themed(driver, set, "setToolsExpanded", json!(expanded)).await?,
            json!({ "ok": null })
        );
        assert_eq!(
            ui_themed(driver, get, "getToolsExpanded", Value::Null).await?,
            json!({ "ok": expanded })
        );
    }
    Ok(())
}

/// Theme lookup and selection, and tool expansion both ways, use the host.
async fn theme_requests(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    theme_lookup(driver).await?;
    theme_selection(driver).await?;
    tool_expansion(driver).await?;
    let control = json!({ "op": "control", "error": "themes 雪\nlocked" });
    assert_eq!(operate(driver, control).await?, json!("controlled"));
    let by_name = json!({ "op": "setThemeName", "name": "dark" });
    assert_eq!(
        operate(driver, json!({ "op": "theme", "call": by_name })).await?,
        json!({ "error": "themes 雪\nlocked" }),
        "a rejection is not a failed selection"
    );
    driver.release_all().await;
    assert_eq!(driver.live(), 0, "no UI or theme resource stays lent");
    Ok(())
}

#[test]
fn theme_lookup_selection_and_tool_expansion_use_host() -> Result<(), String> {
    on_both_adapters!(theme_requests)
}

/// Every thinking level and the shell mode reach the host's border colors, and each answer is
/// the host's own bytes for the palette the retained theme reads.
async fn border_colors(driver: &mut impl Driver) -> Result<(), String> {
    retain_themes(driver, &["dark", "quantized"]).await?;
    let rows = dark_replies(&["thinkingBorder", "bashModeBorder"]);
    assert_eq!(rows.len(), 7, "six thinking levels and the shell mode");
    for row in &rows {
        let answer = reached(driver, 0, "dark", &row.method, &row.operands).await?;
        assert_eq!(answer, row.answer, "{} {}", row.method, row.operands);
    }
    let quantized = corpus().palettes.remove("quantized").unwrap().replies;
    let quantized: Vec<_> = quantized
        .into_iter()
        .filter(|reply| reply.method.ends_with("Border"))
        .collect();
    assert_eq!(
        quantized.len(),
        7,
        "the other palette records its own bytes"
    );
    for row in &quantized {
        let answer = reached(driver, 1, "quantized", &row.method, &row.operands).await?;
        assert_eq!(
            answer, row.answer,
            "quantized {} {}",
            row.method, row.operands
        );
        assert!(
            !rows.iter().any(|dark| dark.answer == row.answer),
            "distinct bytes per palette"
        );
    }
    let level = themed(driver, 0, "thinkingBorder", &json!(["extreme", "x"])).await?;
    assert_eq!(
        level,
        json!({ "error": "unknown thinking level" }),
        "levels are typed"
    );
    release_themes(driver).await
}

#[test]
fn theme_border_helpers_forward_every_level() -> Result<(), String> {
    on_both_adapters!(border_colors)
}

/// The receipts, factory and disposal effects and answer lines of the transcript since
/// `from`; when a host releases a removed widget's captures is its own choice.
fn widget_events(driver: &impl Driver, from: usize) -> Vec<String> {
    driver.transcript()[from..]
        .iter()
        .filter(|line| {
            line.starts_with("ui ")
                || line.starts_with("entry widget-factory")
                || line.starts_with("entry widget-dispose")
                || line.starts_with("entry ui-answer")
        })
        .cloned()
        .collect()
}

/// Shows a labelled factory widget and returns what happened, in order.
async fn show_widget(
    driver: &mut impl Driver,
    label: &str,
    placement: Option<&str>,
    dispose: bool,
) -> Result<Vec<String>, String> {
    let from = driver.transcript().len();
    let operation = json!({ "op": "widgetFactory", "key": "w", "label": label,
        "placement": placement, "dispose": dispose });
    operate(driver, operation).await?;
    Ok(widget_events(driver, from))
}

/// The lines one factory widget operation records, in order.
fn shown(placement: &str, disposed: Option<&str>, built: &str) -> Vec<String> {
    let mut lines = vec![format!("ui 0 setWidgetFactory [\"w\",\"{placement}\"]")];
    lines.extend(disposed.map(|label| format!("entry widget-dispose \"{label}\"")));
    lines.push(format!("entry widget-factory \"{built}\""));
    lines.push("entry ui-answer {\"ok\":true}".to_owned());
    lines
}

/// Replacing a widget disposes the old one before the new factory runs and before the setter
/// returns, whether it keeps or moves its placement.
async fn widget_replacement(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        show_widget(driver, "A", None, true).await?,
        shown("AboveEditor", None, "A")
    );
    assert_eq!(
        show_widget(driver, "B", None, true).await?,
        shown("AboveEditor", Some("A"), "B")
    );
    let moved = show_widget(driver, "C", Some("belowEditor"), false).await?;
    assert_eq!(moved, shown("BelowEditor", Some("B"), "C"));
    assert_eq!(
        driver.widget("w", 24).await?,
        ["C 24"],
        "the factory's capture stays usable"
    );
    assert_eq!(
        show_widget(driver, "D", None, true).await?,
        shown("AboveEditor", None, "D")
    );
    Ok(())
}

/// Removal disposes the widget and clears its key, and captures release only after removal.
async fn widget_removal(driver: &mut impl Driver) -> Result<(), String> {
    let from = driver.transcript().len();
    let remove = json!({ "op": "acquire", "call": { "op": "setWidget", "key": "w" } });
    operate(driver, remove).await?;
    let removed = widget_events(driver, from);
    assert_eq!(
        removed[1..],
        [
            "entry widget-dispose \"D\"",
            "entry ui-answer {\"ok\":true}"
        ]
    );
    assert!(
        driver.widget("w", 24).await.is_err(),
        "removal clears the key"
    );
    driver.release_all().await;
    let transcript = driver.transcript();
    for label in ["A", "B", "C", "D"] {
        let position = |line: String| transcript.iter().position(|it| *it == line);
        let released = position(format!("entry widget-released \"{label}\""));
        let built = position(format!("entry widget-factory \"{label}\""));
        assert!(
            released > built,
            "{label} releases its capture after it was replaced"
        );
    }
    Ok(())
}

/// Factory widgets keep their replacement, disposal and release order on both adapters.
async fn widget_factories(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    widget_replacement(driver).await?;
    widget_removal(driver).await
}

#[test]
fn widget_factory_disposes_before_new_factory_returns() -> Result<(), String> {
    on_both_adapters!(widget_factories)
}

/// Sets the header to a labelled factory component, or restores it, and returns the
/// receipts, factory and disposal effects it caused.
async fn set_header(driver: &mut impl Driver, label: Option<&str>) -> Result<Vec<String>, String> {
    let from = driver.transcript().len();
    operate(driver, json!({ "op": "header", "label": label })).await?;
    Ok(widget_events(driver, from))
}

/// The receipt, disposal, factory and answer lines of one header change.
fn header_change(factory: bool, disposed: Option<&str>, built: Option<&str>) -> Vec<String> {
    let mut lines = vec![format!("ui 0 setHeader [{factory}]")];
    lines.extend(disposed.map(|label| format!("entry widget-dispose \"{label}\"")));
    lines.extend(built.map(|label| format!("entry widget-factory \"{label}\"")));
    lines.push("entry ui-answer {\"ok\":true}".to_owned());
    lines
}

/// A header container of a banner followed by `header`.
fn banner(header: &str) -> Vec<String> {
    vec!["banner".to_owned(), header.to_owned()]
}

/// Before the built-in header exists, setting a header runs no factory.
async fn early_header(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        set_header(driver, Some("early")).await?,
        header_change(true, None, None)
    );
    assert_eq!(
        driver.header(10).await?,
        (Vec::new(), None),
        "no header yet"
    );
    Ok(())
}

/// Each replacement takes the current header's slot after disposing the custom header it
/// replaces, and restoring brings the built-in header back to that slot.
async fn header_in_place(driver: &mut impl Driver) -> Result<(), String> {
    let ready = json!({ "op": "control", "header": "ready" });
    assert_eq!(operate(driver, ready).await?, json!("controlled"));
    assert_eq!(
        set_header(driver, Some("A")).await?,
        header_change(true, None, Some("A"))
    );
    assert_eq!(
        driver.header(10).await?,
        (banner("custom"), Some(vec!["A 10".to_owned()]))
    );
    assert_eq!(
        set_header(driver, Some("B")).await?,
        header_change(true, Some("A"), Some("B"))
    );
    assert_eq!(
        driver.header(5).await?,
        (banner("custom"), Some(vec!["B 5".to_owned()]))
    );
    assert_eq!(
        set_header(driver, None).await?,
        header_change(false, Some("B"), None)
    );
    assert_eq!(
        driver.header(5).await?,
        (banner("built-in"), None),
        "restored in place"
    );
    Ok(())
}

/// A custom header with no header slot to take goes first.
async fn detached_header(driver: &mut impl Driver) -> Result<(), String> {
    let detached = json!({ "op": "control", "header": "detached" });
    assert_eq!(operate(driver, detached).await?, json!("controlled"));
    assert_eq!(
        set_header(driver, Some("C")).await?,
        header_change(true, None, Some("C"))
    );
    let first = vec!["custom".to_owned(), "banner".to_owned()];
    assert_eq!(
        driver.header(3).await?,
        (first, Some(vec!["C 3".to_owned()])),
        "added first"
    );
    Ok(())
}

/// The header keeps its early, in-place and first-slot states on both adapters.
async fn header_slots(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    early_header(driver).await?;
    header_in_place(driver).await?;
    detached_header(driver).await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn header_factory_preserves_early_and_retained_states() -> Result<(), String> {
    on_both_adapters!(header_slots)
}

/// Sets the footer to a labelled factory component, or restores it, and returns the
/// receipts, factory and disposal effects it caused.
async fn set_footer(driver: &mut impl Driver, label: Option<&str>) -> Result<Vec<String>, String> {
    let from = driver.transcript().len();
    operate(driver, json!({ "op": "footer", "label": label })).await?;
    Ok(widget_events(driver, from))
}

/// The receipt, disposal, factory and answer lines of one footer change.
fn footer_change(factory: bool, disposed: Option<&str>, built: Option<&str>) -> Vec<String> {
    let lines = header_change(factory, disposed, built);
    lines
        .iter()
        .map(|line| line.replace("setHeader", "setFooter"))
        .collect()
}

/// Supplies the footer metadata the host reports from now on.
async fn footer_metadata(driver: &mut impl Driver, metadata: Value) -> Result<(), String> {
    let control = json!({ "op": "control", "footer": metadata });
    assert_eq!(operate(driver, control).await?, json!("controlled"));
    Ok(())
}

/// The labels of the footers whose branch listeners ran since `from`.
fn branch_changes(driver: &impl Driver, from: usize) -> Vec<String> {
    driver.transcript()[from..]
        .iter()
        .filter_map(|line| line.strip_prefix("entry branch-changed "))
        .map(str::to_owned)
        .collect()
}

/// A footer reads the current metadata on every render.
async fn live_footer(driver: &mut impl Driver) -> Result<(), String> {
    let first = json!({ "branch": "main", "statuses": [["a", "1"]], "providers": 2.0 });
    footer_metadata(driver, first).await?;
    assert_eq!(
        set_footer(driver, Some("F1")).await?,
        footer_change(true, None, Some("F1"))
    );
    let rendered = driver.footer(20).await?;
    assert_eq!(
        rendered,
        Some(vec![json!(["F1", "main", [["a", "1"]], 2.0]).to_string()])
    );
    let next = json!({ "branch": null, "statuses": [["a", "1"], ["b", ""]], "providers": 0.0 });
    footer_metadata(driver, next).await?;
    let current = json!(["F1", null, [["a", "1"], ["b", ""]], 0.0]).to_string();
    let rendered = driver.footer(20).await?;
    assert_eq!(
        rendered,
        Some(vec![current]),
        "the retained footer reads the new metadata"
    );
    Ok(())
}

/// Branch listeners stay live until unsubscribed, and replacing or restoring the footer
/// disposes the old one first.
async fn footer_replacement(driver: &mut impl Driver) -> Result<(), String> {
    let from = driver.transcript().len();
    assert_eq!(driver.branch_change().await?, [json!({ "ok": null })]);
    assert_eq!(branch_changes(driver, from), ["\"F1\""]);
    let replaced = set_footer(driver, Some("F2")).await?;
    let expected = footer_change(true, Some("F1"), Some("F2"));
    assert_eq!(
        replaced, expected,
        "the old footer is disposed before the new factory runs"
    );
    let unbranch = json!({ "op": "unbranch", "label": "F1" });
    assert_eq!(operate(driver, unbranch).await?, json!({ "ok": true }));
    let from = driver.transcript().len();
    driver.branch_change().await?;
    assert_eq!(
        branch_changes(driver, from),
        ["\"F2\""],
        "only the subscribed listener runs"
    );
    assert_eq!(
        set_footer(driver, None).await?,
        footer_change(false, Some("F2"), None)
    );
    assert_eq!(driver.footer(20).await?, None, "the host footer is back");
    Ok(())
}

/// Footer factories keep live read-only metadata and replacement order on both adapters.
async fn footer_factories(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    live_footer(driver).await?;
    footer_replacement(driver).await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn footer_factory_keeps_live_readonly_metadata() -> Result<(), String> {
    on_both_adapters!(footer_factories)
}

/// The number of transcript lines equal to `line`.
fn count(driver: &impl Driver, line: &str) -> usize {
    driver.transcript().iter().filter(|it| *it == line).count()
}

/// Disposal, the host's release of its component and the author's own alias are separate:
/// removal disposes once, the alias keeps rendering after the host released its owner, and
/// the captures release once, from their destructor, when the last alias drops.
async fn component_release(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let kept = json!({ "op": "keptWidget", "label": "K" });
    assert_eq!(operate(driver, kept).await?, json!({ "ok": true }));
    let remove = json!({ "op": "acquire", "call": { "op": "setWidget", "key": "w" } });
    operate(driver, remove).await?;
    driver.release("widget w").await;
    assert_eq!(
        count(driver, "entry widget-dispose \"K\""),
        1,
        "removal disposed once"
    );
    assert_eq!(
        count(driver, "entry widget-released \"K\""),
        0,
        "the author's alias keeps the capture"
    );
    let rendered = operate(driver, json!({ "op": "renderKept", "width": 7 })).await?;
    assert_eq!(
        rendered,
        json!({ "ok": [["K 7"]] }),
        "the surviving alias still renders"
    );
    assert_eq!(
        operate(driver, json!({ "op": "dropKept" })).await?,
        json!("dropped")
    );
    assert_eq!(
        count(driver, "entry widget-released \"K\""),
        1,
        "released once, from its destructor"
    );
    assert_eq!(
        count(driver, "entry widget-dispose \"K\""),
        1,
        "release is not a second disposal"
    );
    Ok(())
}

#[test]
fn component_release_is_not_dispose() -> Result<(), String> {
    on_both_adapters!(component_release)
}

/// What one painter rendering read about its row.
async fn painted(
    driver: &mut impl Driver,
    slot: &str,
    info: &Value,
    result: Option<&Value>,
) -> Result<Value, String> {
    let run = super::scenario::ToolRenderRun {
        tool: "painter",
        slot,
        info,
        result,
    };
    let lines = driver.render_tool(run, 30).await?;
    assert_eq!(lines.len(), 1, "one line per rendering");
    serde_json::from_str(&lines[0]).map_err(|error| error.to_string())
}

/// A row's info: its call id, arguments so far and every flag.
fn row(id: &str, args: Value, flags: [bool; 6]) -> Value {
    let [started, complete, partial, expanded, images, error] = flags;
    let mut info = json!({ "toolCallId": id, "cwd": "/work", "executionStarted": started,
        "argsComplete": complete, "isPartial": partial, "expanded": expanded,
        "showImages": images, "isError": error });
    info["args"] = args;
    info
}

/// Both renderers read every field of their row, the row's typed state is shared by both
/// slots and kept apart from other rows, and each slot sees its own previous component.
async fn tool_rows(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    assert_eq!(
        operate(driver, json!({ "op": "painter" })).await?,
        json!({ "ok": true })
    );
    let first = row(
        "c1",
        json!({ "a": 1 }),
        [false, false, false, true, false, false],
    );
    let call = painted(driver, "call", &first, None).await?;
    let expected = json!({ "slot": "call", "args": { "a": 1 }, "argsSeen": { "a": 1 },
        "id": "c1", "cwd": "/work", "started": false, "complete": false, "partial": false,
        "expanded": true, "images": false, "error": false, "renders": 1, "last": false });
    assert_eq!(call, expected);
    let next = row(
        "c1",
        json!({ "a": 1, "b": 2 }),
        [true, true, true, false, true, true],
    );
    let call = painted(driver, "call", &next, None).await?;
    assert_eq!(
        (call["renders"].clone(), call["last"].clone()),
        (json!(2), json!(true))
    );
    assert_eq!(call["args"], json!({ "a": 1, "b": 2 }), "current arguments");
    result_slot(driver, &next).await?;
    other_rows(driver).await?;
    driver.release_all().await;
    Ok(())
}

/// The result slot shares the row's state but keeps its own previous component.
async fn result_slot(driver: &mut impl Driver, next: &Value) -> Result<(), String> {
    let result = json!({ "content": [{ "type": "text", "text": "done" }], "details": { "n": 1 } });
    let rendered = painted(driver, "result", next, Some(&result)).await?;
    assert_eq!(
        (rendered["renders"].clone(), rendered["last"].clone()),
        (json!(3), json!(false))
    );
    assert_eq!(rendered["result"], result);
    assert_eq!(rendered["options"], json!([false, true]));
    let rendered = painted(driver, "result", next, Some(&result)).await?;
    assert_eq!(
        rendered["last"],
        json!(true),
        "the result slot keeps its own component"
    );
    Ok(())
}

/// Another row starts with its own state, and a renderer can redraw its row.
async fn other_rows(driver: &mut impl Driver) -> Result<(), String> {
    let other = row("c2", json!({}), [false; 6]);
    assert_eq!(
        painted(driver, "call", &other, None).await?["renders"],
        json!(1)
    );
    let redraw = row("c1", json!({ "invalidate": true }), [false; 6]);
    painted(driver, "call", &redraw, None).await?;
    assert!(
        driver
            .transcript()
            .iter()
            .any(|line| line == "invalidate c1"),
        "the row redraws"
    );
    Ok(())
}

#[test]
fn tool_render_context_keeps_every_field_per_row() -> Result<(), String> {
    on_both_adapters!(tool_rows)
}

/// A result rendered before any call rendering, as history replays it, still reads the
/// current arguments and flags, and a later rendering reads updated arguments.
async fn result_first(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    operate(driver, json!({ "op": "painter" })).await?;
    let result = json!({ "content": [], "details": null, "isError": true });
    for flags in [
        [true, true, false, true, false, true],
        [true, true, true, false, true, false],
    ] {
        let info = row("h1", json!({ "replayed": flags[2] }), flags);
        let rendered = painted(driver, "result", &info, Some(&result)).await?;
        assert_eq!(rendered["args"], json!({ "replayed": flags[2] }));
        let seen = [
            &rendered["expanded"],
            &rendered["images"],
            &rendered["error"],
        ];
        assert_eq!(seen, [&json!(flags[3]), &json!(flags[4]), &json!(flags[5])]);
        assert_eq!(rendered["options"], json!([flags[3], flags[2]]));
    }
    let info = row("h1", json!({ "later": true }), [true; 6]);
    let call = painted(driver, "call", &info, None).await?;
    assert_eq!(
        (call["args"].clone(), call["last"].clone()),
        (json!({ "later": true }), json!(false))
    );
    assert_eq!(call["renders"], json!(3), "the replayed row kept its state");
    driver.release_all().await;
    Ok(())
}

#[test]
fn tool_renderer_result_first_and_history_keep_current_inputs() -> Result<(), String> {
    on_both_adapters!(result_first)
}

/// A returned callback remains callable, reports errors and updates the same retained view.
async fn factory_expanded(driver: &mut impl Driver, width: Option<f64>) -> Result<(), String> {
    let shown = operate(
        driver,
        json!({"op":"factoryCapabilities", "width":width, "expandable":true}),
    )
    .await?;
    assert_eq!(shown, json!({"ok":true}));
    assert_eq!(
        driver.widget_capabilities("capabilities", true).await?,
        (width, true)
    );
    assert_eq!(
        driver.widget("capabilities", 13).await?,
        ["expanded=true width=13"]
    );
    assert_eq!(
        driver
            .widget_capabilities("capabilities", false)
            .await
            .unwrap_err(),
        "expansion rejected 雪\n"
    );
    assert_eq!(
        driver.widget("capabilities", 9).await?,
        ["expanded=false width=9"]
    );
    assert_eq!(
        driver.widget_capabilities("capabilities", true).await?,
        (width, true)
    );
    Ok(())
}

/// Optional capabilities remain absent rather than becoming zero or no-op callbacks.
async fn factory_absent(driver: &mut impl Driver) -> Result<(), String> {
    let shown = operate(
        driver,
        json!({"op":"factoryCapabilities", "width":null, "expandable":false}),
    )
    .await?;
    assert_eq!(shown, json!({"ok":true}));
    assert_eq!(
        driver.widget_capabilities("capabilities", true).await?,
        (None, false)
    );
    assert_eq!(
        driver.widget("capabilities", 7).await?,
        ["expanded=false width=7"]
    );
    Ok(())
}

/// Both adapters preserve literal widths, independent absence and repeated callback effects.
async fn factory_capabilities(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    for width in [None, Some(0.0), Some(-3.5), Some(37.25)] {
        factory_expanded(driver, width).await?;
    }
    factory_absent(driver).await?;
    let emissions: Vec<_> = driver
        .bus_emissions()
        .into_iter()
        .filter(|(channel, _)| channel == "factory-expanded")
        .map(|(_, value)| value)
        .collect();
    assert_eq!(
        emissions,
        [
            true, false, true, true, false, true, true, false, true, true, false, true
        ]
        .map(|flag| json!(flag))
    );
    operate(
        driver,
        json!({"op":"acquire", "call":{"op":"setWidget", "key":"capabilities"}}),
    )
    .await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn factory_component_preserves_width_and_retained_expansion() -> Result<(), String> {
    on_both_adapters!(factory_capabilities)
}

/// A standalone theme has no UI dependency and retained aliases read later publications.
async fn global_theme(driver: &mut impl Driver) -> Result<(), String> {
    retain_themes(driver, &[]).await?;
    driver.run_plain_command("theme", "global").await?;
    assert_eq!(observe(driver, 0).await?, recorded("dark"));
    driver.run_plain_command("theme", "global").await?;
    driver
        .run_plain_command("theme", r#"publish {"theme":"dark","palette":"quantized"}"#)
        .await?;
    assert_eq!(observe(driver, 0).await?, recorded("quantized"));
    assert_eq!(observe(driver, 1).await?, recorded("quantized"));
    release_themes(driver).await
}

#[test]
fn standalone_theme_keeps_live_aliases_without_ui() -> Result<(), String> {
    on_both_adapters!(global_theme)
}

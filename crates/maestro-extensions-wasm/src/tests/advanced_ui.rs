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

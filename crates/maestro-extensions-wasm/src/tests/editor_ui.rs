//! Keybindings, editors and completion providers an extension uses, on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::basic_ui::{install, operate};
use super::scenario::Driver;

/// Runs one widget script and returns each step's answer, failing on a failed step, with the
/// host's keybinding requests.
async fn answers(
    driver: &mut impl Driver,
    steps: Value,
) -> Result<(Vec<Value>, Vec<String>), String> {
    let from = driver.transcript().len();
    let answer = operate(driver, json!({ "op": "widgets", "steps": steps })).await?;
    let seen = driver.transcript()[from..]
        .iter()
        .filter(|line| {
            ["keys ", "hint ", "getKeybindings", "getEditorTheme"]
                .iter()
                .any(|p| line.starts_with(p))
        })
        .cloned()
        .collect();
    let outcomes = answer.as_array().cloned().ok_or("expected step outcomes")?;
    let answers = outcomes
        .into_iter()
        .map(|outcome| {
            outcome
                .get("ok")
                .cloned()
                .ok_or_else(|| outcome.to_string())
        })
        .collect::<Result<_, _>>()?;
    Ok((answers, seen))
}

/// Requests on a manager kept as `name`.
fn on(name: &str, requests: &[Value]) -> Vec<Value> {
    requests
        .iter()
        .map(|request| {
            let mut request = request.clone();
            request["do"] = json!("keys");
            request["name"] = json!(name);
            request
        })
        .collect()
}

/// The host's keybindings, a new manager with user bindings and a file, a created one, one of
/// supplied definitions and one without a file keep their bindings, conflicts and reloads.
async fn keybindings(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let user = json!([
        ["app.demo", "f3"],
        ["tui.select.up", ["f3", "f3"]],
        ["other", "z"]
    ]);
    let steps = keybinding_steps(&user);
    let (answers, seen) = super::editor_ui::answers(driver, json!(steps)).await?;
    let expected = keybinding_answers(user);
    assert_eq!(answers, expected);
    assert!(
        seen.contains(&r#"keys 2 new [[],"/home/a/keybindings.json"]"#.to_owned()),
        "{seen:?}"
    );
    assert!(
        seen.contains(&r#"keys 1 setUserBindings [["app.demo",null]]"#.to_owned()),
        "{seen:?}"
    );
    operate(
        driver,
        json!({ "op": "widgets", "steps": [{ "do": "forget" }] }),
    )
    .await?;
    driver.release_all().await;
    Ok(())
}

/// The keybindings script, given the new manager's user bindings.
fn keybinding_steps(user: &Value) -> Vec<Value> {
    let mut steps = vec![json!({ "do": "keybindings", "name": "g", "source": "global" })];
    steps.extend(on(
        "g",
        &[
            json!({ "op": "getKeys", "id": "app.demo" }),
            json!({ "op": "matches", "data": "f1", "id": "app.demo" }),
            json!({ "op": "matches", "data": "x", "id": "app.demo" }),
            json!({ "op": "getDefinition", "id": "app.demo" }),
            json!({ "op": "getDefinition", "id": "unknown" }),
        ],
    ));
    steps.push(json!({ "do": "keybindings", "name": "k", "source": "new", "user": user, "path": "/k.json" }));
    steps.extend(on(
        "k",
        &[
            json!({ "op": "getKeys", "id": "tui.select.up" }),
            json!({ "op": "getConflicts" }),
            json!({ "op": "getUserBindings" }),
            json!({ "op": "getResolvedBindings" }),
            json!({ "op": "reload" }),
            json!({ "op": "getUserBindings" }),
            json!({ "op": "getEffectiveConfig" }),
            json!({ "op": "setUserBindings", "user": [["app.demo", null]] }),
            json!({ "op": "getKeys", "id": "app.demo" }),
        ],
    ));
    steps.push(json!({ "do": "keybindings", "name": "c", "source": "create", "dir": "/home/a" }));
    steps.push(
        json!({ "do": "keybindings", "name": "d", "source": "definitions",
        "definitions": [["x.one", ["a", "b"], "One"]] }),
    );
    steps.extend(on(
        "d",
        &[
            json!({ "op": "getKeys", "id": "x.one" }),
            json!({ "op": "getDefinition", "id": "x.one" }),
        ],
    ));
    steps.push(json!({ "do": "keybindings", "name": "p", "source": "new" }));
    steps.extend(on(
        "p",
        &[
            json!({ "op": "reload" }),
            json!({ "op": "getUserBindings" }),
        ],
    ));
    steps
}

/// The answers of the keybindings script, given the new manager's user bindings.
fn keybinding_answers(user: Value) -> Vec<Value> {
    vec![
        json!(true),
        json!(["ctrl+x", "f1"]),
        json!(true),
        json!(false),
        json!([["ctrl+x", "f1"], "Demo"]),
        Value::Null,
        json!(true),
        json!(["f3"]),
        json!([["f3", ["app.demo", "tui.select.up"]]]),
        user,
        json!([["tui.select.up", ["f3"]], ["app.demo", ["f3"]]]),
        json!(true),
        json!([["app.demo", "f2"]]),
        json!([["tui.select.up", ["up"]], ["app.demo", ["f2"]]]),
        json!(true),
        json!(["ctrl+x", "f1"]),
        json!(true),
        json!(true),
        json!(["a", "b"]),
        json!([["a", "b"], "One"]),
        json!(true),
        json!(true),
        json!([]),
    ]
}

#[test]
fn keybinding_facade_keeps_registered_configuration() -> Result<(), String> {
    on_both_adapters!(keybindings)
}

/// Keybinding hints read the host's keybindings and live theme, and the editor styles are the
/// host's.
async fn hints(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let steps = json!([
        { "do": "hint", "id": "app.demo" },
        { "do": "hint", "id": "app.demo", "description": "demo 雪" },
        { "do": "hint", "key": "esc", "description": "back" },
        { "do": "hint", "id": "none" },
        { "do": "editorTheme", "text": "x" },
    ]);
    let (answers, seen) = answers(driver, steps).await?;
    let expected = [
        json!("ctrl+x/f1"),
        json!("<dim>ctrl+x/f1<muted> demo 雪"),
        json!("<dim>esc<muted> back"),
        json!(""),
        json!(["<borderMuted>x", "<selectedText>x"]),
    ];
    assert_eq!(answers, expected);
    assert!(
        seen.contains(&r#"hint [null,"esc","back"]"#.to_owned()),
        "{seen:?}"
    );
    driver.release_all().await;
    Ok(())
}

#[test]
fn keybinding_hints_and_editor_styles_come_from_the_host() -> Result<(), String> {
    on_both_adapters!(hints)
}

/// The authored handler calls a script made so far, forgotten once read.
async fn views(driver: &mut impl Driver) -> Result<Value, String> {
    let (answers, _) = answers(driver, json!([{ "do": "views" }])).await?;
    Ok(answers[0].clone())
}

/// One request on the editor kept as `name`.
fn edit(name: &str, op: &str, operand: Option<(&str, Value)>) -> Value {
    let mut step = json!({ "do": "edit", "name": name, "op": op });
    if let Some((key, value)) = operand {
        step[key] = value;
    }
    step
}

/// The host editor script.
fn host_editor_steps() -> Value {
    json!([
        { "do": "editor", "name": "e", "theme": "host", "options": [2, 7] },
        { "do": "render", "name": "e", "width": 10 },
        { "do": "on", "name": "e", "slot": "change", "label": "changed" },
        { "do": "on", "name": "e", "slot": "submit", "label": "sent" },
        edit("e", "setText", Some(("text", json!("hi")))),
        { "do": "input", "name": "e", "data": "!" },
        edit("e", "getLines", None),
        edit("e", "insertText", Some(("text", json!("\nyo")))),
        edit("e", "getCursor", None),
        edit("e", "getExpandedText", None),
        { "do": "input", "name": "e", "data": "enter" },
        edit("e", "getText", None),
        edit("e", "setDisableSubmit", Some(("flag", json!(true)))),
        { "do": "input", "name": "e", "data": "enter" },
        edit("e", "disableSubmit", None),
        edit("e", "setPaddingX", Some(("number", json!(3.7)))),
        edit("e", "getPaddingX", None),
        edit("e", "setAutocompleteMaxVisible", Some(("number", json!(50)))),
        edit("e", "getAutocompleteMaxVisible", None),
        edit("e", "isShowingAutocomplete", None),
        edit("e", "setFocused", Some(("flag", json!(true)))),
        edit("e", "focused", None),
        edit("e", "addToHistory", Some(("text", json!("old")))),
        edit("e", "setBorderColor", Some(("text", json!("b")))),
        { "do": "render", "name": "e", "width": 4 },
    ])
}

/// The host editor keeps its text, cursor, history, options and focus, runs its change and
/// submission handlers and paints its border with a lent or authored style.
async fn host_editor(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let from = driver.transcript().len();
    let steps = host_editor_steps();
    let (answers, _) = answers(driver, steps).await?;
    let (done, unit) = (json!(true), Value::Null);
    let expected = vec![
        done.clone(),
        json!(["<borderMuted>edit|10", ""]),
        done.clone(),
        done.clone(),
        unit.clone(),
        done.clone(),
        json!(["hi!"]),
        unit.clone(),
        json!([1, 2]),
        json!("hi!\nyo"),
        done.clone(),
        json!(""),
        unit.clone(),
        done,
        json!(true),
        unit.clone(),
        json!(3.0),
        unit.clone(),
        json!(20.0),
        json!(false),
        unit.clone(),
        json!(true),
        unit.clone(),
        unit,
        json!(["b(edit|4)", ""]),
    ];
    assert_eq!(answers, expected);
    let calls = json!([
        "changed hi",
        "changed hi!",
        "changed hi!\nyo",
        "sent hi!\nyo"
    ]);
    assert_eq!(views(driver).await?, calls);
    let built = driver.transcript()[from..]
        .iter()
        .any(|line| line.contains("new editor") && line.contains(r#""paddingX":2.0"#));
    assert!(built, "{:?}", &driver.transcript()[from..]);
    operate(
        driver,
        json!({ "op": "widgets", "steps": [{ "do": "forget" }] }),
    )
    .await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn host_editor_keeps_text_options_and_handlers() -> Result<(), String> {
    on_both_adapters!(host_editor)
}

/// The application's custom editor offers input to its interceptor, then its special keys and
/// application actions, before editing.
async fn custom_editor(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let input = |data: &str| json!({ "do": "input", "name": "c", "data": data });
    let on =
        |slot: &str, label: &str| json!({ "do": "on", "name": "c", "slot": slot, "label": label });
    let action = |action: &str, label: &str| json!({ "do": "action", "name": "c", "action": action, "label": label });
    let steps = json!([
        { "do": "keybindings", "name": "k", "source": "new" },
        { "do": "editor", "name": "c", "keys": "k", "theme": "authored" },
        action("app.demo", "demo"),
        action("app.interrupt", "interrupt"),
        input("f1"),
        input("esc"),
        on("escape", "escape"),
        input("esc"),
        input("ctrl+d"),
        on("ctrlD", "exit"),
        input("ctrl+d"),
        on("pasteImage", "paste"),
        input("ctrl+v"),
        on("shortcut", "short"),
        input("ctrl+k"),
        input("f1"),
        action("app.demo", "demo2"),
        input("f1"),
        on("change", "changed"),
        input("z"),
        { "do": "render", "name": "c", "width": 6 },
        edit("c", "getText", None),
    ]);
    let (answers, _) = answers(driver, steps).await?;
    let mut expected = vec![json!(true); 20];
    expected.extend([json!(["border(edit|6)", "z"]), json!("z")]);
    assert_eq!(answers, expected);
    let calls = json!([
        "demo",
        "interrupt",
        "escape",
        "exit",
        "paste",
        "short ctrl+k",
        "short f1",
        "demo",
        "short f1",
        "demo2",
        "short z",
        "changed z"
    ]);
    assert_eq!(views(driver).await?, calls);
    operate(
        driver,
        json!({ "op": "widgets", "steps": [{ "do": "forget" }] }),
    )
    .await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn custom_editor_offers_input_to_shortcuts_and_actions_first() -> Result<(), String> {
    on_both_adapters!(custom_editor)
}

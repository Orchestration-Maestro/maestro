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

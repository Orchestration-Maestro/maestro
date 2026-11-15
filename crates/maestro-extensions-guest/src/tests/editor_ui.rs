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

/// Installs the editor `kind` names and checks the factory the host answers back.
async fn install_editor(driver: &mut impl Driver, kind: &str) -> Result<Value, String> {
    let answer = operate(driver, json!({ "op": "editorFactory", "kind": kind })).await?;
    answer.get("ok").cloned().ok_or_else(|| answer.to_string())
}

/// Runs host editor requests in order and returns each described answer.
async fn requests(driver: &mut impl Driver, requests: Value) -> Result<Vec<Value>, String> {
    let mut answers = Vec::new();
    for request in requests.as_array().cloned().ok_or("expected requests")? {
        answers.push(driver.editor_request(request).await?);
    }
    Ok(answers)
}

/// The lent text callback calls and host editor requests recorded since `from`.
fn editor_lines(driver: &impl Driver, from: usize) -> Vec<String> {
    driver.transcript()[from..]
        .iter()
        .filter(|line| line.starts_with("text ") || line.starts_with("editor built"))
        .cloned()
        .collect()
}

/// An authored editor keeps its text and lent callbacks, reports every optional operation as
/// absent, and returns its change handler to the host.
async fn plain_editor(driver: &mut impl Driver) -> Result<(), String> {
    use super::renderer_host::{Call, Reply};
    let from = driver.transcript().len();
    assert_eq!(
        install_editor(driver, "plain").await?,
        json!({ "same": true })
    );
    let answers = requests(
        driver,
        json!([
            { "op": "setOnSubmit", "callback": "submit" },
            { "op": "setOnChange", "callback": "change" },
            { "op": "setText", "text": "λ\n雪" },
            { "op": "getText" },
        ]),
    )
    .await?;
    assert_eq!(
        answers,
        [
            json!("done"),
            json!("done"),
            json!("done"),
            json!({ "text": "λ\n雪" })
        ]
    );
    for data in ["!", "enter"] {
        assert_eq!(
            driver.editor_component(Call::Input(data)).await?,
            Reply::Done
        );
    }
    assert_eq!(
        driver.editor_component(Call::Render(9)).await?,
        Reply::Lines(vec!["plain|9".into(), "λ".into(), "雪!".into()])
    );
    plain_absence(driver).await?;
    assert_eq!(
        editor_lines(driver, from),
        [
            "editor built custom=null",
            r#"text change "λ\n雪""#,
            r#"text change "λ\n雪!""#,
            r#"text submit "λ\n雪!""#,
            r#"text change "probe""#,
        ]
    );
    Ok(())
}

/// A plain authored editor answers each optional operation as absent and returns its change
/// handler until it is removed.
async fn plain_absence(driver: &mut impl Driver) -> Result<(), String> {
    let optional = requests(
        driver,
        json!([
            { "op": "getExpandedText" },
            { "op": "borderColor" },
            { "op": "setBorderColor", "role": "borderAccent" },
            { "op": "setPaddingX", "padding": 2.0 },
            { "op": "addToHistory", "text": "past" },
            { "op": "insertTextAtCursor", "text": "ins" },
            { "op": "setAutocompleteMaxVisible", "max": 3.0 },
            { "op": "onChange" },
            { "op": "setOnChange", "callback": null },
            { "op": "onChange" },
            { "op": "setText", "text": "after" },
        ]),
    )
    .await?;
    let mut expected = vec![json!("absent"); 7];
    expected.extend([
        json!({ "handler": true }),
        json!("done"),
        json!({ "handler": false }),
        json!("done"),
    ]);
    assert_eq!(optional, expected);
    Ok(())
}

/// The host's editor supports every optional operation and keeps its own border style.
async fn host_editor_factory(driver: &mut impl Driver) -> Result<(), String> {
    use super::renderer_host::{Call, Reply};
    let from = driver.transcript().len();
    assert_eq!(
        install_editor(driver, "host").await?,
        json!({ "same": true })
    );
    let answers = requests(
        driver,
        json!([
            { "op": "setOnChange", "callback": "change" },
            { "op": "setText", "text": "a" },
            { "op": "getExpandedText" },
            { "op": "borderColor" },
            { "op": "setBorderColor", "role": "borderAccent" },
            { "op": "borderColor" },
            { "op": "setPaddingX", "padding": 2.0 },
            { "op": "addToHistory", "text": "past" },
            { "op": "insertTextAtCursor", "text": "!" },
            { "op": "setAutocompleteMaxVisible", "max": 4.0 },
            { "op": "onChange" },
            { "op": "getText" },
        ]),
    )
    .await?;
    assert_eq!(
        answers,
        [
            json!("done"),
            json!("done"),
            json!({ "text": "a" }),
            json!({ "style": "<borderMuted>border" }),
            json!("done"),
            json!({ "style": "<borderAccent>border" }),
            json!("done"),
            json!("done"),
            json!("done"),
            json!("done"),
            json!({ "handler": true }),
            json!({ "text": "a!" }),
        ]
    );
    assert_eq!(
        driver.editor_component(Call::Render(6)).await?,
        Reply::Lines(vec!["<borderAccent>edit|6".into(), "a!".into()])
    );
    assert_eq!(
        editor_lines(driver, from),
        [
            "editor built custom=null",
            r#"text change "a""#,
            r#"text change "a!""#,
            r#"text change "probe""#,
        ]
    );
    Ok(())
}

/// An authored editor on the custom editor shares that editor's application actions with the
/// host and passes on the input it does not keep.
async fn custom_editor_factory(driver: &mut impl Driver) -> Result<(), String> {
    use super::renderer_host::{Call, Reply};
    let from = driver.transcript().len();
    assert_eq!(
        install_editor(driver, "custom").await?,
        json!({ "same": true })
    );
    for data in ["x", "q", "f1"] {
        assert_eq!(
            driver.editor_component(Call::Input(data)).await?,
            Reply::Done
        );
    }
    let answers = requests(
        driver,
        json!([{ "op": "getText" }, { "op": "setPaddingX", "padding": 1.0 }]),
    )
    .await?;
    assert_eq!(answers, [json!({ "text": "q" }), json!("done")]);
    assert_eq!(
        editor_lines(driver, from),
        [r#"editor built custom=["app.demo"]"#]
    );
    let effects: Vec<_> = driver
        .bus_emissions()
        .into_iter()
        .filter(|(channel, _)| channel == "modal-kept" || channel == "custom-action")
        .collect();
    assert_eq!(
        effects,
        [
            ("modal-kept".to_owned(), json!("x")),
            ("custom-action".to_owned(), json!("app.demo")),
        ]
    );
    Ok(())
}

/// Factories keep their identity and absence, failures keep their message, and every
/// authored editor keeps its callbacks, text and optional appearance across the transport.
async fn editor_factories(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    assert_eq!(
        install_editor(driver, "none").await?,
        json!({ "current": false })
    );
    plain_editor(driver).await?;
    host_editor_factory(driver).await?;
    custom_editor_factory(driver).await?;
    let failing = operate(driver, json!({ "op": "editorFactory", "kind": "failing" })).await?;
    assert_eq!(failing, json!({ "error": "editor factory failed 雪\n" }));
    assert_eq!(
        install_editor(driver, "none").await?,
        json!({ "current": false })
    );
    let requested: Vec<_> = driver
        .transcript()
        .iter()
        .filter(|line| line.contains("EditorComponent"))
        .cloned()
        .collect();
    let set = |shown: bool| format!("ui 0 setEditorComponent [{shown}]");
    let get = "ui 0 getEditorComponent []".to_owned();
    assert_eq!(
        requested,
        [
            set(false),
            get.clone(),
            set(true),
            get.clone(),
            set(true),
            get.clone(),
            set(true),
            get.clone(),
            set(true),
            set(false),
            get,
        ]
    );
    driver.release_all().await;
    Ok(())
}

#[test]
fn editor_factory_retains_callbacks_text_and_appearance() -> Result<(), String> {
    on_both_adapters!(editor_factories)
}

/// The requests an editor another extension's factory built receives through a wrapping
/// factory.
fn foreign_requests() -> Value {
    json!([
        { "op": "setOnChange", "callback": "change" },
        { "op": "setText", "text": "a" },
        { "op": "insertTextAtCursor", "text": "!" },
        { "op": "getText" },
        { "op": "getExpandedText" },
        { "op": "borderColor" },
        { "op": "setBorderColor", "role": "borderAccent" },
        { "op": "borderColor" },
        { "op": "setPaddingX", "padding": 2.0 },
        { "op": "addToHistory", "text": "past" },
        { "op": "setAutocompleteMaxVisible", "max": 4.0 },
        { "op": "onChange" },
        { "op": "setAutocompleteProvider" },
        { "op": "setOnSubmit", "callback": "submit" },
    ])
}

/// What an editor another extension's factory built answers to [`foreign_requests`]; a plain
/// one lacks every optional operation.
fn foreign_answers(custom: bool) -> Vec<Value> {
    let optional = |answer: Value| if custom { answer } else { json!("absent") };
    vec![
        json!("done"),
        json!("done"),
        optional(json!("done")),
        json!({ "text": if custom { "a!" } else { "a" } }),
        optional(json!({ "text": "a!" })),
        optional(json!({ "style": "<borderMuted>border" })),
        optional(json!("done")),
        optional(json!({ "style": "<borderAccent>border" })),
        optional(json!("done")),
        optional(json!("done")),
        optional(json!("done")),
        json!({ "handler": true }),
        optional(json!("done")),
        json!("done"),
    ]
}

/// The editor and text callback lines of [`foreign_editor`], given the text typed.
fn foreign_lines(custom: bool, typed: &str) -> Vec<String> {
    let shown = if custom { "[]" } else { "null" };
    let mut lines = vec![
        format!("editor built custom={shown}"),
        r#"text change "a""#.to_owned(),
    ];
    lines.extend(custom.then(|| r#"text change "a!""#.to_owned()));
    lines.push(r#"text foreign "probe""#.to_owned());
    lines.push(format!("text change {}", json!(typed)));
    lines
}

/// Types into the editor another extension's factory built, checks its rendering and returns
/// its text.
async fn foreign_input(driver: &mut impl Driver, custom: bool) -> Result<&'static str, String> {
    use super::renderer_host::{Call, Reply};
    assert_eq!(
        driver.editor_component(Call::Input("q")).await?,
        Reply::Done
    );
    let (typed, border) = if custom {
        ("a!q", "borderAccent")
    } else {
        ("aq", "borderMuted")
    };
    assert_eq!(
        driver.editor_component(Call::Render(6)).await?,
        Reply::Lines(vec![format!("<{border}>edit|6"), typed.into()])
    );
    Ok(typed)
}

/// Wraps the editor factory of `kind` another extension set and checks what the editor it
/// builds answers, renders and calls back.
async fn foreign_editor(driver: &mut impl Driver, kind: &str) -> Result<(), String> {
    let control = json!({ "op": "control", "foreign": kind });
    assert_eq!(operate(driver, control).await?, json!("controlled"));
    let from = driver.transcript().len();
    assert_eq!(
        install_editor(driver, "wrap").await?,
        json!({ "same": true })
    );
    let custom = kind == "custom";
    assert_eq!(
        requests(driver, foreign_requests()).await?,
        foreign_answers(custom)
    );
    let typed = foreign_input(driver, custom).await?;
    let built = driver.transcript()[from..]
        .iter()
        .filter(|line| line.starts_with("foreign built"))
        .count();
    assert_eq!(built, 1);
    assert_eq!(editor_lines(driver, from), foreign_lines(custom, typed));
    assert_eq!(
        install_editor(driver, "none").await?,
        json!({ "current": false })
    );
    Ok(())
}

/// Another extension's editor factory is current, builds through the host and keeps its
/// absent optional operations; without one there is none to wrap.
async fn foreign_editor_factories(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let missing = operate(driver, json!({ "op": "editorFactory", "kind": "wrap" })).await?;
    assert_eq!(missing, json!({ "error": "no editor factory to wrap" }));
    foreign_editor(driver, "custom").await?;
    foreign_editor(driver, "plain").await?;
    driver.release_all().await;
    Ok(())
}

#[test]
fn foreign_editor_factory_builds_through_the_host() -> Result<(), String> {
    on_both_adapters!(foreign_editor_factories)
}

/// Runs provider requests in order and returns each described answer.
async fn provided(driver: &mut impl Driver, requests: Value) -> Result<Vec<Value>, String> {
    let mut answers = Vec::new();
    for request in requests.as_array().cloned().ok_or("expected requests")? {
        answers.push(driver.provider_request(request).await?);
    }
    Ok(answers)
}

/// A stacked provider's suggestions keep absence and force, and it keeps the signal after
/// the request returns.
async fn stacked_suggestions(driver: &mut impl Driver) -> Result<(), String> {
    let mut suggestions = |force: Value, aborted: bool, line: &str| {
        let signal = driver.lend_signal(aborted)?;
        let request = json!({ "op": "suggestions", "lines": [line], "cursor": [0, 4],
            "force": force, "signal": signal });
        Ok::<_, String>((request, signal))
    };
    let requests = [
        suggestions(json!(null), false, "src/")?,
        suggestions(json!(false), false, "src/")?,
        suggestions(json!(true), true, "src/")?,
        suggestions(json!(null), false, "mine")?,
    ];
    let kept = requests[3].1;
    let mut answers = Vec::new();
    for (request, _) in requests {
        answers.push(driver.provider_request(request).await?);
    }
    let first = json!(["first", "FIRST", null]);
    let base = json!(["base", "Base", "host"]);
    let expected = [
        json!({ "suggestions": [[base, first], "src/"] }),
        json!({ "suggestions": null }),
        json!({ "suggestions": [[base, first], "src/"] }),
        json!({ "suggestions": [[first], "mine"] }),
    ];
    assert_eq!(answers, expected);
    let trigger = json!({ "op": "trigger", "lines": ["kept"], "cursor": [0, 0] });
    assert_eq!(
        driver.provider_request(trigger.clone()).await?,
        json!({ "trigger": false })
    );
    driver.abort_signal(kept)?;
    assert_eq!(
        driver.provider_request(trigger).await?,
        json!({ "trigger": true })
    );
    Ok(())
}

/// A stacked provider receives the current provider, applies its own candidates, passes
/// others and file-completion decisions on, and editors keep or lack provider installation.
async fn stacked_provider(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let from = driver.transcript().len();
    let stacked = operate(driver, json!({ "op": "stackProvider", "name": "first" })).await?;
    assert_eq!(stacked, json!({ "ok": true }));
    stacked_suggestions(driver).await?;
    let answers = provided(
        driver,
        json!([
            { "op": "apply", "lines": ["src/"], "cursor": [0, 4],
              "item": ["base", "Base", "host"], "prefix": "src/" },
            { "op": "apply", "lines": ["x"], "cursor": [0, 1],
              "item": ["first", "FIRST", null], "prefix": "x" },
            { "op": "trigger", "lines": ["@x"], "cursor": [0, 2] },
            { "op": "trigger", "lines": ["x"], "cursor": [0, 1] },
        ]),
    )
    .await?;
    let expected = [
        json!({ "applied": [["base"], 0, 4] }),
        json!({ "applied": [["x->first"], 0, 2] }),
        json!({ "trigger": true }),
        json!({ "trigger": null }),
    ];
    assert_eq!(answers, expected);
    let host: Vec<_> = driver.transcript()[from..]
        .iter()
        .filter(|line| line.starts_with("provider ") || line.contains("addAutocompleteProvider"))
        .cloned()
        .collect();
    assert_eq!(
        host,
        [
            "ui 0 addAutocompleteProvider []",
            "provider stacked",
            r#"provider suggestions [["src/"],[0,4],null,false]"#,
            r#"provider suggestions [["src/"],[0,4],false,false]"#,
            r#"provider suggestions [["src/"],[0,4],true,true]"#,
            r#"provider apply [["src/"],[0,4],"base","src/"]"#,
            r#"provider trigger [["@x"],[0,2]]"#,
            r#"provider trigger [["x"],[0,1]]"#,
        ]
    );
    for (kind, answer) in [("plain", "absent"), ("host", "done")] {
        assert_eq!(install_editor(driver, kind).await?, json!({ "same": true }));
        let installed = json!({ "op": "setAutocompleteProvider" });
        assert_eq!(driver.editor_request(installed).await?, json!(answer));
    }
    let factories: Vec<_> = driver
        .bus_emissions()
        .into_iter()
        .filter(|(channel, _)| channel == "provider-factory")
        .collect();
    assert_eq!(factories, [("provider-factory".to_owned(), json!("first"))]);
    driver.release_all().await;
    Ok(())
}

#[test]
fn autocomplete_keeps_async_signal_and_wrapper_stack() -> Result<(), String> {
    on_both_adapters!(stacked_provider)
}

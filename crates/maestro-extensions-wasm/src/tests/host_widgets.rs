//! Host widgets an extension builds, changes and renders, on both adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::basic_ui::{install, operate};
use super::scenario::Driver;

/// Runs one widget script and returns each step's outcome, with the host's widget requests.
async fn script(
    driver: &mut impl Driver,
    steps: Value,
) -> Result<(Vec<Value>, Vec<String>), String> {
    let from = driver.transcript().len();
    let answer = operate(driver, json!({ "op": "widgets", "steps": steps })).await?;
    let seen = driver.transcript()[from..]
        .iter()
        .filter(|line| {
            ["widget ", "style ", "highlight ", "getMarkdownTheme"]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .cloned()
        .collect();
    let outcomes = answer.as_array().cloned().ok_or("expected step outcomes")?;
    Ok((outcomes, seen))
}

/// A successful step outcome.
fn ok(value: Value) -> Value {
    Value::Object(serde_json::Map::from_iter([("ok".to_owned(), value)]))
}

/// Rendered rows as a successful outcome.
fn rows(rows: &[&str]) -> Value {
    ok(json!(rows))
}

/// Forgets the widgets and releases every registration.
async fn finish(driver: &mut impl Driver) -> Result<(), String> {
    operate(
        driver,
        json!({ "op": "widgets", "steps": [{ "do": "forget" }] }),
    )
    .await?;
    driver.release_all().await;
    Ok(())
}

/// Text keeps its literal text and padding, follows each change, renders at each width and
/// paints with authored, host and failing backgrounds.
async fn text(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "text", "name": "t", "text": "hi 雪", "padding": [1, 0] },
            { "do": "render", "name": "t", "width": 3 },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "render", "name": "t", "width": 24 },
            { "do": "setText", "name": "t", "text": "" },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "setBg", "name": "t", "bg": "A" },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "setBg", "name": "t", "bg": "host:heading" },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "setBg", "name": "t", "bg": "fail:F" },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "setBg", "name": "t" },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "invalidate", "name": "t" },
            { "do": "text", "name": "u", "text": "styled", "padding": [0, 2], "bg": "B" },
            { "do": "render", "name": "u", "width": 5 },
        ]),
    )
    .await?;
    let expected = [
        ok(json!(true)),
        rows(&["text hi 雪 1 0|3"]),
        rows(&["text hi 雪 1 0|8"]),
        rows(&["text hi 雪 1 0|24"]),
        ok(json!(true)),
        rows(&["text  1 0|8"]),
        ok(json!(true)),
        rows(&["A(text  1 0|8)"]),
        ok(json!(true)),
        rows(&["<heading>text  1 0|8"]),
        ok(json!(true)),
        json!({ "error": "F failed on text  1 0|8" }),
        ok(json!(true)),
        rows(&["text  1 0|8"]),
        ok(json!(true)),
        ok(json!(true)),
        rows(&["B(text styled 0 2|5)"]),
    ];
    assert_eq!(outcomes, expected);
    text_requests(&seen);
    finish(driver).await
}

/// The host received the text's literal operands, and a host style returned as itself.
fn text_requests(seen: &[String]) {
    assert_eq!(seen[0], r#"widget 1 new text ["hi 雪",[1,0],0,null,null]"#);
    for line in [
        "widget 1 setText \"\"",
        "widget 1 setBackground \"heading\"",
        "widget 1 setBackground null",
        "widget 1 invalidate",
    ] {
        assert!(seen.contains(&line.to_owned()), "{line}: {seen:?}");
    }
    let authored = "widget 2 new text [\"styled\",[0,2],0,\"authored ";
    assert!(
        seen.iter().any(|line| line.starts_with(authored)),
        "{seen:?}"
    );
}

#[test]
fn host_text_keeps_mutation_styles_and_width() -> Result<(), String> {
    on_both_adapters!(text)
}

/// A spacer keeps each row count it is given.
async fn spacer(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "spacer", "name": "s", "lines": 0 },
            { "do": "render", "name": "s", "width": 5 },
            { "do": "setLines", "name": "s", "lines": 3 },
            { "do": "render", "name": "s", "width": 5 },
            { "do": "setLines", "name": "s", "lines": 1 },
            { "do": "invalidate", "name": "s" },
            { "do": "render", "name": "s", "width": 2 },
        ]),
    )
    .await?;
    let expected = [
        ok(json!(true)),
        rows(&[]),
        ok(json!(true)),
        rows(&["", "", ""]),
        ok(json!(true)),
        ok(json!(true)),
        rows(&[""]),
    ];
    assert_eq!(outcomes, expected);
    assert_eq!(seen[0], r#"widget 1 new spacer ["",[0,0],0,null,null]"#);
    assert!(seen.contains(&"widget 1 setLines 3".to_owned()), "{seen:?}");
    finish(driver).await
}

#[test]
fn host_spacer_keeps_height_and_changes() -> Result<(), String> {
    on_both_adapters!(spacer)
}

/// Host and authored children render in insertion order, removal takes the first occurrence,
/// an absent child changes nothing and a removed child stays usable.
async fn container(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, _) = script(
        driver,
        json!([
            { "do": "container", "name": "c" },
            { "do": "text", "name": "t", "text": "a", "padding": [0, 0] },
            { "do": "view", "name": "v", "widgets": [] },
            { "do": "add", "name": "c", "child": "t" },
            { "do": "add", "name": "c", "child": "v" },
            { "do": "add", "name": "c", "child": "t" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "remove", "name": "c", "child": "t" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "remove", "name": "c", "child": "v" },
            { "do": "remove", "name": "c", "child": "v" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "add", "name": "c", "child": "v" },
            { "do": "invalidate", "name": "c" },
            { "do": "views" },
            { "do": "clear", "name": "c" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "render", "name": "t", "width": 4 },
        ]),
    )
    .await?;
    let added = ok(json!(true));
    let expected = [
        added.clone(),
        added.clone(),
        added.clone(),
        added.clone(),
        added.clone(),
        added.clone(),
        rows(&["text a 0 0|6", "view v", "text a 0 0|6"]),
        added.clone(),
        rows(&["view v", "text a 0 0|6"]),
        added.clone(),
        added.clone(),
        rows(&["text a 0 0|6"]),
        added.clone(),
        added.clone(),
        ok(json!(["v render 6", "v render 6", "v invalidate"])),
        added,
        rows(&[]),
        rows(&["text a 0 0|4"]),
    ];
    assert_eq!(outcomes, expected);
    finish(driver).await
}

#[test]
fn host_container_keeps_mixed_children_and_live_order() -> Result<(), String> {
    on_both_adapters!(container)
}

/// A box paints its row through its background, keeps host and authored children and drops
/// them on clear.
async fn boxed(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "box", "name": "b", "padding": [2, 1], "bg": "B" },
            { "do": "text", "name": "t", "text": "in", "padding": [0, 0] },
            { "do": "view", "name": "v", "widgets": ["t"] },
            { "do": "add", "name": "b", "child": "t" },
            { "do": "add", "name": "b", "child": "v" },
            { "do": "render", "name": "b", "width": 9 },
            { "do": "setBg", "name": "b" },
            { "do": "render", "name": "b", "width": 9 },
            { "do": "remove", "name": "b", "child": "t" },
            { "do": "render", "name": "b", "width": 9 },
            { "do": "clear", "name": "b" },
            { "do": "render", "name": "b", "width": 9 },
        ]),
    )
    .await?;
    let done = ok(json!(true));
    let expected = [
        done.clone(),
        done.clone(),
        done.clone(),
        done.clone(),
        done.clone(),
        rows(&["B(box 2 1|9)", "text in 0 0|9", "view v", "text in 0 0|9"]),
        done.clone(),
        rows(&["box 2 1|9", "text in 0 0|9", "view v", "text in 0 0|9"]),
        done.clone(),
        rows(&["box 2 1|9", "view v", "text in 0 0|9"]),
        done,
        rows(&["box 2 1|9"]),
    ];
    assert_eq!(outcomes, expected);
    assert!(
        seen[0].starts_with(r#"widget 1 new box ["",[2,1],0,"authored "#),
        "{seen:?}"
    );
    finish(driver).await
}

#[test]
fn host_box_keeps_children_and_background_callbacks() -> Result<(), String> {
    on_both_adapters!(boxed)
}

/// An authored component renders host text and Markdown while the host renders it, and an
/// invalidation reaches the widgets it renders.
async fn composed(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "text", "name": "t", "text": "inner", "padding": [1, 1] },
            { "do": "markdown", "name": "m", "text": "# T", "padding": [0, 0], "theme": "host" },
            { "do": "view", "name": "v", "widgets": ["t", "m"] },
            { "do": "container", "name": "c" },
            { "do": "add", "name": "c", "child": "v" },
            { "do": "render", "name": "c", "width": 10 },
            { "do": "invalidate", "name": "c" },
            { "do": "views" },
        ]),
    )
    .await?;
    let done = ok(json!(true));
    let expected = [
        done.clone(),
        done.clone(),
        done.clone(),
        done.clone(),
        done.clone(),
        rows(&[
            "view v",
            "text inner 1 1|10",
            "<heading>md # T|10",
            "<hl rust>fn main() {}",
            "<hl rust>let x = 1;",
        ]),
        done,
        ok(json!(["v render 10", "v invalidate"])),
    ];
    assert_eq!(outcomes, expected);
    let order: Vec<_> = seen
        .iter()
        .filter(|line| line.contains("render") || line.contains("invalidate"))
        .cloned()
        .collect();
    let expected = [
        "widget 3 render 10",
        "widget 1 render 10",
        "widget 2 render 10",
        "widget 3 invalidate",
        "widget 1 invalidate",
        "widget 2 invalidate",
    ];
    assert_eq!(
        order, expected,
        "the nested renders run inside the outer one"
    );
    finish(driver).await
}

#[test]
fn guest_component_renders_retained_host_widgets() -> Result<(), String> {
    on_both_adapters!(composed)
}

/// Markdown keeps its text, the host's or an authored theme and its base style, and the
/// host's styles stay callable.
async fn markdown(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "markdown", "name": "m", "text": "# T", "padding": [1, 0], "theme": "host",
              "style": ["host:bold", null, ["bold", "underline"]] },
            { "do": "render", "name": "m", "width": 7 },
            { "do": "markdown", "name": "n", "text": "x", "padding": [0, 0], "theme": "authored" },
            { "do": "render", "name": "n", "width": 7 },
            { "do": "setText", "name": "n", "text": "" },
            { "do": "invalidate", "name": "n" },
            { "do": "render", "name": "n", "width": 2 },
            { "do": "apply", "role": "heading", "text": "z 雪" },
        ]),
    )
    .await?;
    let done = ok(json!(true));
    let expected = [
        done.clone(),
        rows(&[
            "<heading>md # T|7",
            "<hl rust>fn main() {}",
            "<hl rust>let x = 1;",
        ]),
        done.clone(),
        rows(&[
            "heading(md x|7)",
            "hl rust(fn main() {})",
            "hl rust(let x = 1;)",
        ]),
        done.clone(),
        done,
        rows(&[
            "heading(md |2)",
            "hl rust(fn main() {})",
            "hl rust(let x = 1;)",
        ]),
        ok(json!("<heading>z 雪")),
    ];
    assert_eq!(outcomes, expected);
    markdown_requests(&seen)?;
    finish(driver).await
}

/// The host received its own styles back, and the authored theme with its indent.
fn markdown_requests(seen: &[String]) -> Result<(), String> {
    let host = seen
        .iter()
        .find(|line| line.starts_with("widget 1 new markdown"))
        .ok_or("no host Markdown")?;
    let operands: Value = serde_json::from_str(&host["widget 1 new markdown ".len()..])
        .map_err(|error| error.to_string())?;
    let roles: Vec<_> = super::widget_host::ROLES
        .iter()
        .map(|role| format!("Host({role:?})"))
        .collect();
    let expected = json!({ "roles": roles, "highlight": "Host(\"hl\")", "indent": null,
        "style": ["Host(\"bold\")", null, ["Bold", "Underline"]] });
    assert_eq!(operands[4], expected, "host styles return as themselves");
    let authored = seen
        .iter()
        .find(|line| line.starts_with("widget 2 new markdown"))
        .ok_or("no authored Markdown")?;
    assert!(authored.contains(r#""indent":"> ""#), "{authored}");
    assert!(
        seen.contains(&"style heading \"z 雪\"".to_owned()),
        "{seen:?}"
    );
    Ok(())
}

#[test]
fn host_markdown_keeps_text_theme_and_explicit_highlighting() -> Result<(), String> {
    on_both_adapters!(markdown)
}

/// A rule paints across the width with the live border color, or with its authored color.
async fn border(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, _) = script(
        driver,
        json!([
            { "do": "border", "name": "d" },
            { "do": "render", "name": "d", "width": 4 },
            { "do": "border", "name": "e", "color": "C" },
            { "do": "render", "name": "e", "width": 3 },
            { "do": "invalidate", "name": "e" },
            { "do": "render", "name": "e", "width": 1 },
        ]),
    )
    .await?;
    let done = ok(json!(true));
    let expected = [
        done.clone(),
        rows(&["<border>────"]),
        done.clone(),
        rows(&["C(───)"]),
        done,
        rows(&["C(─)"]),
    ];
    assert_eq!(outcomes, expected);
    finish(driver).await
}

#[test]
fn host_dynamic_border_keeps_color_callback() -> Result<(), String> {
    on_both_adapters!(border)
}

/// The authored callback calls a script recorded, in order.
async fn views(driver: &mut impl Driver) -> Result<Value, String> {
    let (outcomes, _) = script(driver, json!([{ "do": "views" }])).await?;
    Ok(outcomes[0]["ok"].clone())
}

/// A selection list keeps its choices, moves and wraps its selection, clamps a selected index,
/// filters by value and runs each handler until it is removed.
async fn select_list(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let items = json!([
        ["a", "Alpha", null],
        ["ab", "Abel", null],
        ["b", "Beta", "desc"]
    ]);
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "select", "name": "s", "items": items, "maxVisible": 5, "theme": "authored" },
            { "do": "render", "name": "s", "width": 20 },
            { "do": "on", "name": "s", "slot": "select", "label": "picked" },
            { "do": "on", "name": "s", "slot": "cancel", "label": "cancelled" },
            { "do": "on", "name": "s", "slot": "change", "label": "moved" },
            { "do": "input", "name": "s", "data": "\u{1b}[B" },
            { "do": "input", "name": "s", "data": "\r" },
            { "do": "input", "name": "s", "data": "\u{1b}" },
            { "do": "filter", "name": "s", "filter": "A" },
            { "do": "selected", "name": "s" },
            { "do": "selectIndex", "name": "s", "index": 5 },
            { "do": "selected", "name": "s" },
            { "do": "selectIndex", "name": "s", "index": -3 },
            { "do": "selected", "name": "s" },
            { "do": "input", "name": "s", "data": "\u{1b}[A" },
            { "do": "filter", "name": "s", "filter": "zz" },
            { "do": "render", "name": "s", "width": 20 },
            { "do": "selected", "name": "s" },
            { "do": "on", "name": "s", "slot": "select" },
            { "do": "filter", "name": "s", "filter": "" },
            { "do": "input", "name": "s", "data": "\r" },
        ]),
    )
    .await?;
    let expected = select_expected();
    assert_eq!(outcomes, expected);
    let calls = json!(["moved ab", "picked ab", "cancelled", "moved ab"]);
    assert_eq!(views(driver).await?, calls);
    assert!(
        seen.contains(&"widget 1 setHandler select false".to_owned()),
        "{seen:?}"
    );
    finish(driver).await
}

/// The outcomes of the selection list script.
fn select_expected() -> Vec<Value> {
    let done = ok(json!(true));
    let mut expected = vec![
        done.clone(),
        rows(&["selectedText(> Alpha)", "  Abel", "  Beta"]),
    ];
    expected.extend(vec![done.clone(); 7]);
    expected.extend([
        ok(json!(["a", "Alpha"])),
        done.clone(),
        ok(json!(["ab", "Abel"])),
    ]);
    expected.extend([
        done.clone(),
        ok(json!(["a", "Alpha"])),
        done.clone(),
        done.clone(),
    ]);
    expected.extend([rows(&[]), ok(Value::Null), done.clone(), done.clone(), done]);
    expected
}

#[test]
fn host_select_list_keeps_selection_and_callbacks() -> Result<(), String> {
    on_both_adapters!(select_list)
}

/// The primary column widths in both orders and the authored truncation reach the host, and
/// the host's styles stay callable.
async fn select_layout(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let items = json!([["a", "Alpha\nnext", "two\nlines"], ["b", "Beta", null]]);
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "select", "name": "l", "items": items, "maxVisible": 2, "theme": "host",
              "widths": [10, 20], "truncate": true },
            { "do": "render", "name": "l", "width": 30 },
            { "do": "select", "name": "m", "items": items, "maxVisible": 2, "theme": "authored",
              "widths": [20, 10] },
            { "do": "applyList", "role": "selectedText", "text": "x 雪" },
        ]),
    )
    .await?;
    let expected = [
        ok(json!(true)),
        rows(&["cut Alpha\nnext 28 32 true", "cut Beta 28 32 false"]),
        ok(json!(true)),
        ok(json!("<selectedText>x 雪")),
    ];
    assert_eq!(outcomes, expected);
    let operands = |key: u32| -> Result<Value, String> {
        let prefix = format!("widget {key} new select ");
        let line = seen
            .iter()
            .find_map(|line| line.strip_prefix(&prefix))
            .ok_or("no selection list")?;
        let operands: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        Ok(operands[4].clone())
    };
    let host = operands(1)?;
    let roles = [
        "selectedPrefix",
        "selectedText",
        "description",
        "scrollInfo",
        "noMatch",
    ];
    let styles: Vec<_> = roles.iter().map(|role| format!("Host({role:?})")).collect();
    assert_eq!(
        host["styles"],
        json!(styles),
        "the host's styles return as themselves"
    );
    assert_eq!(
        [&host["min"], &host["max"], &host["truncate"]],
        [&json!(10), &json!(20), &json!(true)]
    );
    let authored = operands(2)?;
    assert_eq!(
        [&authored["min"], &authored["max"], &authored["truncate"]],
        [&json!(20), &json!(10), &json!(false)]
    );
    assert_eq!(
        authored["items"],
        json!([["a", "Alpha\nnext"], ["b", "Beta"]])
    );
    finish(driver).await
}

#[test]
fn host_select_layout_forwards_every_option() -> Result<(), String> {
    on_both_adapters!(select_layout)
}

/// A settings list cycles values, reports changes, updates values silently, opens a submenu
/// that receives input and closes with or without a value, and reports cancellation.
async fn settings_list(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let items = json!([
        ["mode", "Mode", "fast", ["fast", "slow"], null],
        ["color", "Color", "red", null, "pick"],
    ]);
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "settings", "name": "t", "items": items, "maxVisible": 5,
              "theme": "authored", "search": true },
            { "do": "render", "name": "t", "width": 9 },
            { "do": "input", "name": "t", "data": "\r" },
            { "do": "updateValue", "name": "t", "id": "mode", "value": "fast" },
            { "do": "input", "name": "t", "data": "\u{1b}[B" },
            { "do": "input", "name": "t", "data": "\r" },
            { "do": "render", "name": "t", "width": 9 },
            { "do": "input", "name": "t", "data": "\r" },
            { "do": "render", "name": "t", "width": 9 },
            { "do": "input", "name": "t", "data": "\r" },
            { "do": "input", "name": "t", "data": "\u{1b}" },
            { "do": "input", "name": "t", "data": "\u{1b}" },
            { "do": "applyRow", "role": "label", "text": "x", "selected": true },
        ]),
    )
    .await?;
    let expected = settings_expected();
    assert_eq!(outcomes, expected);
    let calls = json!([
        "change mode slow",
        "pick opened red",
        "pick input \"\\r\"",
        "change color picked",
        "pick opened picked",
        "pick input \"\\u001b\"",
        "cancel settings",
    ]);
    assert_eq!(views(driver).await?, calls);
    assert!(
        seen.contains(&r#"widget 1 updateValue ["mode","fast"]"#.to_owned()),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|line| line.contains(r#""search":true"#)),
        "{seen:?}"
    );
    finish(driver).await
}

/// The outcomes of the settings list script.
fn settings_expected() -> Vec<Value> {
    let done = ok(json!(true));
    let mut expected = vec![
        done.clone(),
        rows(&[
            "label(Mode,true)",
            "value(fast,true)",
            "label(Color,false)",
            "value(red,false)",
        ]),
    ];
    expected.extend(vec![done.clone(); 4]);
    expected.extend([rows(&["submenu pick red 9"]), done.clone()]);
    expected.push(rows(&[
        "label(Mode,false)",
        "value(fast,false)",
        "label(Color,true)",
        "value(picked,true)",
    ]));
    expected.extend(vec![done; 3]);
    expected.push(ok(json!("<label true>x")));
    expected
}

#[test]
fn host_settings_list_keeps_search_cycles_and_submenus() -> Result<(), String> {
    on_both_adapters!(settings_list)
}

/// A loader built from the lent terminal and theme reports cancellation through its signal and
/// handler only while it is cancellable, and stops on disposal.
async fn loader(driver: &mut impl Driver) -> Result<(), String> {
    install(driver).await?;
    let (outcomes, seen) = script(
        driver,
        json!([
            { "do": "loader", "name": "l", "message": "Working 雪" },
            { "do": "render", "name": "l", "width": 10 },
            { "do": "aborted", "name": "l" },
            { "do": "on", "name": "l", "slot": "abort", "label": "aborted" },
            { "do": "input", "name": "l", "data": "x" },
            { "do": "input", "name": "l", "data": "\u{1b}" },
            { "do": "aborted", "name": "l" },
            { "do": "dispose", "name": "l" },
            { "do": "loader", "name": "f", "message": "Fixed", "cancellable": false },
            { "do": "input", "name": "f", "data": "\u{1b}" },
            { "do": "aborted", "name": "f" },
        ]),
    )
    .await?;
    let done = ok(json!(true));
    let expected = [
        done.clone(),
        rows(&["loader Working 雪"]),
        ok(json!(false)),
        done.clone(),
        done.clone(),
        done.clone(),
        ok(json!(true)),
        done.clone(),
        done.clone(),
        done,
        ok(json!(false)),
    ];
    assert_eq!(outcomes, expected);
    assert_eq!(views(driver).await?, json!(["aborted"]));
    let loaders: Vec<_> = seen
        .iter()
        .filter(|line| line.contains("new loader"))
        .collect();
    let first = r#"{"theme":"dark","message":"Working 雪","cancellable":null}"#;
    assert!(loaders[0].contains(first), "{loaders:?}");
    assert!(loaders[1].contains(r#""cancellable":false"#), "{loaders:?}");
    assert!(
        seen.iter().any(|line| line.ends_with(" dispose")),
        "{seen:?}"
    );
    finish(driver).await
}

#[test]
fn host_loader_keeps_cancel_signal_and_disposal() -> Result<(), String> {
    on_both_adapters!(loader)
}

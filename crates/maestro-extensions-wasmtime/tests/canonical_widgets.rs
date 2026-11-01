//! Host widgets a component builds without a UI render through the toolkit widgets.
#![cfg(test)]
use maestro_extensions::{ExtensionRunner, Local, create_event_bus_for};
use maestro_tui::{
    Box as Boxed, Component, Container, Markdown, MarkdownOptions, MarkdownTheme, Spacer,
    TerminalImage, Text, editor_component::BorderColor,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Loads the bridge component with no UI bound and runs `body` against its runner.
fn with_bridge(body: impl AsyncFnOnce(&ExtensionRunner<Local>)) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        body(&runner).await;
        bus.clear();
    });
}

/// Runs one widget script and returns each step's outcome.
async fn script(runner: &ExtensionRunner<Local>, steps: &Value) -> Vec<Value> {
    let handler = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let answer = handler(
        format!("ui widgets {steps}"),
        runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string();
    serde_json::from_str(&answer).unwrap()
}

/// A successful outcome.
fn ok(value: Value) -> Value {
    Value::Object(serde_json::Map::from_iter([("ok".to_owned(), value)]))
}

/// The rows of a toolkit component as a successful outcome.
fn rows(component: &dyn Component, width: usize) -> Value {
    ok(json!(component.render(width)))
}

/// An authored style as the bridge writes it.
fn authored(label: &'static str) -> BorderColor {
    Rc::new(move |text: &str| format!("{label}({text})"))
}

/// The authored component of the bridge: its name, then each widget's rows.
struct View(Vec<Rc<dyn Component>>);
impl Component for View {
    fn render(&self, width: usize) -> Vec<String> {
        let mut rows = vec!["view v".to_owned()];
        for widget in &self.0 {
            rows.extend(widget.render(width));
        }
        rows
    }
}

#[test]
fn canonical_text_and_spacer_render_through_the_toolkit_without_a_ui() {
    with_bridge(async |runner| {
        let steps = json!([
            { "do": "text", "name": "t", "text": "hi 雪 there", "padding": [1, 0] },
            { "do": "render", "name": "t", "width": 3 },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "setText", "name": "t", "text": "" },
            { "do": "render", "name": "t", "width": 8 },
            { "do": "setText", "name": "t", "text": "x y" },
            { "do": "setBg", "name": "t", "bg": "A" },
            { "do": "render", "name": "t", "width": 6 },
            { "do": "setBg", "name": "t", "bg": "fail:F" },
            { "do": "render", "name": "t", "width": 6 },
            { "do": "setBg", "name": "t" },
            { "do": "render", "name": "t", "width": 6 },
            { "do": "spacer", "name": "s", "lines": 3 },
            { "do": "render", "name": "s", "width": 4 },
            { "do": "setLines", "name": "s", "lines": 0 },
            { "do": "render", "name": "s", "width": 4 },
        ]);
        let outcomes = script(runner, &steps).await;
        assert_eq!(outcomes, text_expected());
    });
}

/// The outcomes of the text and spacer script, from the same toolkit widgets.
fn text_expected() -> Vec<Value> {
    let text = Text::new("hi 雪 there".into(), 1, 0, None);
    let first = rows(&text, 3);
    let second = rows(&text, 8);
    text.set_text(String::new());
    let empty = rows(&text, 8);
    text.set_text("x y".into());
    text.set_custom_bg_fn(Some(authored("A")));
    let painted = rows(&text, 6);
    let seen = Rc::new(RefCell::new(None));
    let first_row = seen.clone();
    text.set_custom_bg_fn(Some(Rc::new(move |row: &str| {
        first_row.borrow_mut().get_or_insert_with(|| row.to_owned());
        row.to_owned()
    })));
    text.render(6);
    let failed = json!({ "error": format!("F failed on {}", seen.borrow().clone().unwrap()) });
    text.set_custom_bg_fn(None);
    let plain = rows(&text, 6);
    let spacer = Spacer::new(3);
    let three = rows(&spacer, 4);
    spacer.set_lines(0);
    let none = rows(&spacer, 4);
    let done = ok(json!(true));
    let expected = [
        done.clone(),
        first,
        second,
        done.clone(),
        empty,
        done.clone(),
        done.clone(),
        painted,
        done.clone(),
        failed,
        done.clone(),
        plain,
        done.clone(),
        three,
        done,
        none,
    ];
    expected.to_vec()
}

#[test]
fn canonical_containers_and_boxes_keep_host_and_authored_children() {
    with_bridge(async |runner| {
        let steps = json!([
            { "do": "container", "name": "c" },
            { "do": "text", "name": "t", "text": "a", "padding": [0, 0] },
            { "do": "view", "name": "v", "widgets": ["t"] },
            { "do": "add", "name": "c", "child": "t" },
            { "do": "add", "name": "c", "child": "v" },
            { "do": "add", "name": "c", "child": "t" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "remove", "name": "c", "child": "t" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "remove", "name": "c", "child": "v" },
            { "do": "remove", "name": "c", "child": "v" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "box", "name": "b", "padding": [2, 1], "bg": "B" },
            { "do": "add", "name": "b", "child": "v" },
            { "do": "render", "name": "b", "width": 9 },
            { "do": "invalidate", "name": "b" },
            { "do": "clear", "name": "b" },
            { "do": "render", "name": "b", "width": 9 },
            { "do": "views" },
        ]);
        let outcomes = script(runner, &steps).await;
        assert_eq!(outcomes, children_expected());
    });
}

/// The outcomes of the container and box script, from the same toolkit widgets.
fn children_expected() -> Vec<Value> {
    let text: Rc<dyn Component> = Rc::new(Text::new("a".into(), 0, 0, None));
    let view: Rc<dyn Component> = Rc::new(View(vec![text.clone()]));
    let container = Container::new();
    container.add_child(text.clone());
    container.add_child(view.clone());
    container.add_child(text.clone());
    let mixed = rows(&container, 6);
    container.remove_child(&text);
    let removed = rows(&container, 6);
    container.remove_child(&view);
    let last = rows(&container, 6);
    let boxed = Boxed::new(2, 1, Some(authored("B")));
    boxed.add_child(view.clone());
    let painted = rows(&boxed, 9);
    boxed.clear();
    let cleared = rows(&boxed, 9);
    let done = ok(json!(true));
    let mut expected = vec![done.clone(); 6];
    expected.extend([
        mixed,
        done.clone(),
        removed,
        done.clone(),
        done.clone(),
        last,
    ]);
    expected.extend([
        done.clone(),
        done.clone(),
        painted,
        done.clone(),
        done,
        cleared,
    ]);
    let views = ["v render 6", "v render 6", "v render 5", "v invalidate"];
    expected.push(ok(json!(views)));
    expected
}

#[test]
fn canonical_nested_host_widgets_return_their_failures() {
    with_bridge(async |runner| {
        let steps = json!([
            { "do": "text", "name": "t", "text": "x y", "padding": [0, 0], "bg": "fail:F" },
            { "do": "container", "name": "c" },
            { "do": "add", "name": "c", "child": "t" },
            { "do": "render", "name": "c", "width": 6 },
            { "do": "text", "name": "u", "text": "x y", "padding": [0, 0], "bg": "fail:F" },
            { "do": "box", "name": "b", "padding": [0, 0] },
            { "do": "add", "name": "b", "child": "u" },
            { "do": "render", "name": "b", "width": 6 },
        ]);
        let outcomes = script(runner, &steps).await;
        let seen = Rc::new(RefCell::new(None));
        let first_row = seen.clone();
        let text = Text::new("x y".into(), 0, 0, None);
        text.set_custom_bg_fn(Some(Rc::new(move |row: &str| {
            first_row.borrow_mut().get_or_insert_with(|| row.to_owned());
            row.to_owned()
        })));
        text.render(6);
        let failed = json!({ "error": format!("F failed on {}", seen.borrow().clone().unwrap()) });
        let done = ok(json!(true));
        let expected = [
            done.clone(),
            done.clone(),
            done.clone(),
            failed.clone(),
            done.clone(),
            done.clone(),
            done,
            failed,
        ];
        assert_eq!(outcomes, expected);
    });
}

/// Markdown options with horizontal padding and no base style.
fn options(padding_x: usize) -> MarkdownOptions {
    MarkdownOptions {
        clean_content: false,
        padding_x,
        padding_y: 0,
        default_text_style: None,
    }
}

/// The authored Markdown theme of the bridge: each role wraps text in its name.
fn authored_theme() -> MarkdownTheme {
    let role = |name: &'static str| -> Box<dyn Fn(&str) -> String> {
        Box::new(move |text| format!("{name}({text})"))
    };
    MarkdownTheme {
        heading: role("heading"),
        link: role("link"),
        link_url: role("linkUrl"),
        code: role("code"),
        code_block: role("codeBlock"),
        code_block_border: role("codeBlockBorder"),
        quote: role("quote"),
        quote_border: role("quoteBorder"),
        hr: role("hr"),
        list_bullet: role("listBullet"),
        bold: role("bold"),
        italic: role("italic"),
        strikethrough: role("strikethrough"),
        underline: role("underline"),
        highlight_code: Some(Box::new(|code, lang| {
            code.lines()
                .map(|line| format!("hl {}({line})", lang.unwrap_or_default()))
                .collect()
        })),
        code_block_indent: Some("> ".to_owned()),
    }
}

#[test]
fn canonical_markdown_and_borders_render_with_host_and_authored_styles() {
    with_bridge(async |runner| {
        let text = "# Title\n\nSome **bold** and `code`.\n\n```rust\nfn main() {}\n```";
        let steps = json!([
            { "do": "markdown", "name": "m", "text": text, "padding": [1, 0], "theme": "host" },
            { "do": "render", "name": "m", "width": 24 },
            { "do": "markdown", "name": "n", "text": text, "padding": [0, 0], "theme": "authored" },
            { "do": "render", "name": "n", "width": 30 },
            { "do": "setText", "name": "n", "text": "- item" },
            { "do": "render", "name": "n", "width": 30 },
            { "do": "view", "name": "v", "widgets": ["m"] },
            { "do": "container", "name": "c" },
            { "do": "add", "name": "c", "child": "v" },
            { "do": "render", "name": "c", "width": 24 },
            { "do": "border", "name": "d" },
            { "do": "render", "name": "d", "width": 4 },
            { "do": "border", "name": "e", "color": "C" },
            { "do": "render", "name": "e", "width": 3 },
            { "do": "apply", "role": "heading", "text": "z" },
        ]);
        let outcomes = script(runner, &steps).await;
        assert_eq!(outcomes, markdown_expected(text));
    });
}

/// The outcomes of the Markdown and rule script, from the same toolkit widgets.
fn markdown_expected(text: &str) -> Vec<Value> {
    let widgets = crate::support::widgets();
    let terminal = || TerminalImage::new(|_| None, || 1);
    let host = Markdown::new(
        text.into(),
        options(1),
        Rc::new(widgets.markdown_theme()),
        terminal(),
    );
    let authored_markdown = Markdown::new(
        text.into(),
        options(0),
        Rc::new(authored_theme()),
        terminal(),
    );
    let first = rows(&authored_markdown, 30);
    authored_markdown.set_text("- item".into());
    let item = rows(&authored_markdown, 30);
    let host: Rc<dyn Component> = Rc::new(host);
    let view = View(vec![host.clone()]);
    let done = ok(json!(true));
    let expected = [
        done.clone(),
        rows(&*host, 24),
        done.clone(),
        first,
        done.clone(),
        item,
        done.clone(),
        done.clone(),
        done.clone(),
        rows(&view, 24),
        done.clone(),
        rows(&*widgets.dynamic_border(None), 4),
        done,
        rows(&*widgets.dynamic_border(Some(authored("C"))), 3),
        ok(json!((widgets.markdown_theme().heading)("z"))),
    ];
    expected.to_vec()
}

/// The authored selection list styles of the bridge: each role wraps text in its name.
fn select_theme() -> maestro_tui::SelectListTheme {
    let role = |name: &'static str| -> BorderColor {
        Rc::new(move |text: &str| format!("{name}({text})"))
    };
    maestro_tui::SelectListTheme {
        selected_prefix: role("selectedPrefix"),
        selected_text: role("selectedText"),
        description: role("description"),
        scroll_info: role("scrollInfo"),
        no_match: role("noMatch"),
    }
}

/// The choices of the selection list scripts.
fn choices() -> Vec<maestro_tui::SelectItem> {
    [
        ("a", "Alpha", None),
        ("ab", "Abel", None),
        ("b", "Beta", Some("desc")),
    ]
    .into_iter()
    .map(|(value, label, description)| maestro_tui::SelectItem {
        value: value.to_owned(),
        label: label.to_owned(),
        description: description.map(str::to_owned),
    })
    .collect()
}

#[test]
fn canonical_select_lists_run_authored_handlers_styles_and_layout() {
    with_bridge(async |runner| {
        let items = json!([
            ["a", "Alpha", null],
            ["ab", "Abel", null],
            ["b", "Beta", "desc"]
        ]);
        let steps = json!([
            { "do": "select", "name": "s", "items": items, "maxVisible": 5, "theme": "authored" },
            { "do": "render", "name": "s", "width": 30 },
            { "do": "on", "name": "s", "slot": "select", "label": "picked" },
            { "do": "on", "name": "s", "slot": "cancel", "label": "cancelled" },
            { "do": "on", "name": "s", "slot": "change", "label": "moved" },
            { "do": "input", "name": "s", "data": "\u{1b}[B" },
            { "do": "input", "name": "s", "data": "\r" },
            { "do": "input", "name": "s", "data": "\u{1b}" },
            { "do": "filter", "name": "s", "filter": "A" },
            { "do": "selectIndex", "name": "s", "index": 9 },
            { "do": "selected", "name": "s" },
            { "do": "select", "name": "l", "items": items, "maxVisible": 2, "theme": "authored",
              "widths": [10, 20], "truncate": true },
            { "do": "render", "name": "l", "width": 30 },
            { "do": "views" },
        ]);
        let outcomes = script(runner, &steps).await;
        let list = maestro_tui::SelectList::new(
            choices(),
            5,
            select_theme(),
            maestro_tui::SelectListLayoutOptions::default(),
        );
        let truncate: Rc<
            dyn for<'a> Fn(maestro_tui::SelectListTruncatePrimaryContext<'a>) -> String,
        > = Rc::new(|context| {
            format!(
                "cut {} {} {} {}",
                context.text, context.max_width, context.column_width, context.is_selected
            )
        });
        let layout = maestro_tui::SelectListLayoutOptions {
            min_primary_column_width: Some(10),
            max_primary_column_width: Some(20),
            truncate_primary: Some(truncate),
        };
        let cut = maestro_tui::SelectList::new(choices(), 2, select_theme(), layout);
        let done = ok(json!(true));
        let mut expected = vec![done.clone(), rows(&list, 30)];
        expected.extend(vec![done.clone(); 8]);
        expected.extend([ok(json!(["ab", "Abel"])), done, rows(&cut, 30)]);
        expected.push(ok(json!(["moved ab", "picked ab", "cancelled"])));
        assert_eq!(outcomes, expected);
    });
}

#[test]
fn canonical_settings_lists_cycle_values_and_open_authored_submenus() {
    with_bridge(async |runner| {
        let items = json!([
            ["mode", "Mode", "fast", ["fast", "slow"], null],
            ["color", "Color", "red", null, "pick"],
        ]);
        let steps = json!([
            { "do": "settings", "name": "t", "items": items, "maxVisible": 5, "theme": "authored" },
            { "do": "render", "name": "t", "width": 40 },
            { "do": "input", "name": "t", "data": "\r" },
            { "do": "render", "name": "t", "width": 40 },
            { "do": "input", "name": "t", "data": "\u{1b}[B" },
            { "do": "input", "name": "t", "data": "\r" },
            { "do": "render", "name": "t", "width": 40 },
            { "do": "input", "name": "t", "data": "\r" },
            { "do": "render", "name": "t", "width": 40 },
            { "do": "updateValue", "name": "t", "id": "mode", "value": "fast" },
            { "do": "render", "name": "t", "width": 40 },
            { "do": "input", "name": "t", "data": "\u{1b}" },
            { "do": "views" },
        ]);
        let outcomes = script(runner, &steps).await;
        let rendered = |index: usize| outcomes[index]["ok"].to_string();
        assert!(rendered(1).contains("> label(Mode"), "{}", rendered(1));
        assert!(rendered(1).contains("value(fast,true)"), "{}", rendered(1));
        assert!(
            rendered(3).contains("value(slow,true)"),
            "a cycle changes the value"
        );
        assert_eq!(
            outcomes[6],
            rows_of(&["submenu pick red 40"]),
            "the submenu renders"
        );
        assert!(
            rendered(8).contains("value(picked,true)"),
            "{}",
            rendered(8)
        );
        assert!(
            rendered(10).contains("value(fast,false)"),
            "an update changes no selection"
        );
        let calls = json!([
            "change mode slow",
            "pick opened red",
            "pick input \"\\r\"",
            "change color picked",
            "cancel settings",
        ]);
        assert_eq!(outcomes[12], ok(calls));
    });
}

/// Rows as a successful outcome.
fn rows_of(rows: &[&str]) -> Value {
    ok(json!(rows))
}

#[test]
fn canonical_loaders_report_cancellation_through_their_signal() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let ui = published_ui();
        runner.set_ui_context(Some(maestro_extensions::ExtensionUIContext::new(
            ui.clone(),
        )));
        let steps = json!([
            { "do": "loader", "name": "l", "message": "Working" },
            { "do": "aborted", "name": "l" },
            { "do": "on", "name": "l", "slot": "abort", "label": "aborted" },
            { "do": "input", "name": "l", "data": "x" },
            { "do": "aborted", "name": "l" },
            { "do": "input", "name": "l", "data": "\u{1b}" },
            { "do": "aborted", "name": "l" },
            { "do": "dispose", "name": "l" },
            { "do": "loader", "name": "f", "message": "Fixed", "cancellable": false },
            { "do": "input", "name": "f", "data": "\u{1b}" },
            { "do": "aborted", "name": "f" },
            { "do": "views" },
        ]);
        let outcomes = script(&runner, &steps).await;
        let done = ok(json!(true));
        let expected = [
            done.clone(),
            ok(json!(false)),
            done.clone(),
            done.clone(),
            ok(json!(false)),
            done.clone(),
            ok(json!(true)),
            done.clone(),
            done.clone(),
            done,
            ok(json!(false)),
            ok(json!(["aborted"])),
        ];
        assert_eq!(outcomes, expected);
        drop(runner);
        bus.clear();
    });
}

/// A UI that builds factories on a terminal with a published theme.
fn published_ui() -> Rc<super::canonical_ui::Widgets> {
    let (tui, _terminal) = super::canonical_ui::screen();
    let state = maestro_theme::ThemeState::new(maestro_theme::ThemeDirectories {
        themes_dir: String::new(),
        custom_themes_dir: String::new(),
    });
    let theme = maestro_theme::Theme::new(
        [],
        [],
        maestro_theme::ColorMode::Truecolor,
        maestro_theme::ThemeOptions::default(),
    );
    state.set_theme_instance(Rc::new(theme.unwrap())).unwrap();
    Rc::new(super::canonical_ui::Widgets {
        tui,
        theme: state.theme(),
        shown: RefCell::default(),
    })
}

#[test]
fn canonical_keybindings_reach_the_host_managers_hints_and_editor_styles() {
    with_bridge(async |runner| {
        let user = json!([["tui.select.up", "f3"], ["tui.select.down", "f3"]]);
        let steps = json!([
            { "do": "keybindings", "name": "g", "source": "global" },
            { "do": "keys", "name": "g", "op": "getKeys", "id": "tui.select.cancel" },
            { "do": "keys", "name": "g", "op": "matches", "data": "\u{1b}", "id": "tui.select.cancel" },
            { "do": "keybindings", "name": "k", "source": "new", "user": user, "path": "/k.json" },
            { "do": "keys", "name": "k", "op": "getKeys", "id": "tui.select.up" },
            { "do": "keys", "name": "k", "op": "getConflicts" },
            { "do": "keys", "name": "k", "op": "reload" },
            { "do": "keys", "name": "k", "op": "getUserBindings" },
            { "do": "keys", "name": "k", "op": "getKeys", "id": "tui.select.up" },
            { "do": "keybindings", "name": "d", "source": "definitions",
              "definitions": [["x.one", ["a", "b", "a"], "One"]], "user": [["x.one", null]] },
            { "do": "keys", "name": "d", "op": "getKeys", "id": "x.one" },
            { "do": "keys", "name": "d", "op": "getDefinition", "id": "x.one" },
            { "do": "keybindings", "name": "p", "source": "new" },
            { "do": "keys", "name": "p", "op": "reload" },
            { "do": "keys", "name": "p", "op": "getUserBindings" },
            { "do": "hint", "id": "tui.select.cancel" },
            { "do": "hint", "id": "tui.select.cancel", "description": "cancel" },
            { "do": "hint", "key": "esc", "description": "back" },
            { "do": "editorTheme", "text": "x" },
        ]);
        let outcomes = script(runner, &steps).await;
        let keys = maestro_tui::get_keybindings().get_keys("tui.select.cancel");
        let joined = keys.join("/");
        let done = ok(json!(true));
        let expected = vec![
            done.clone(),
            ok(json!(keys)),
            ok(json!(true)),
            done.clone(),
            ok(json!(["f3"])),
            ok(json!([["f3", ["tui.select.up", "tui.select.down"]]])),
            done.clone(),
            ok(json!([["tui.select.up", "f2"]])),
            ok(json!(["f2"])),
            done.clone(),
            ok(json!(["a", "b"])),
            ok(json!([["a", "b", "a"], "One"])),
            done.clone(),
            done,
            ok(json!([])),
            ok(json!(joined)),
            ok(json!(format!("[{joined}] cancel"))),
            ok(json!("[esc] back")),
            ok(json!(["x", "x"])),
        ];
        assert_eq!(outcomes, expected);
        retained_keybinding_configs(runner).await;
    });
}

/// Retained created and pathless managers keep independent overrides and owned snapshots.
async fn retained_keybinding_configs(runner: &ExtensionRunner<Local>) {
    let steps = json!([
        { "do": "keybindings", "name": "c", "source": "create", "dir": "/explicit" },
        { "do": "keys", "name": "c", "op": "setUserBindings", "user": [["tui.select.up", "f7"]] },
        { "do": "keys", "name": "c", "op": "getUserBindings" },
        { "do": "keys", "name": "c", "op": "getResolvedBindings" },
        { "do": "keys", "name": "c", "op": "getEffectiveConfig" },
        { "do": "keys", "name": "c", "op": "reload" },
        { "do": "keys", "name": "c", "op": "getUserBindings" },
        { "do": "keys", "name": "c", "op": "getResolvedBindings" },
        { "do": "keys", "name": "c", "op": "getEffectiveConfig" },
        { "do": "keys", "name": "k", "op": "getKeys", "id": "tui.select.up" },
        { "do": "keys", "name": "p", "op": "setUserBindings", "user": [["tui.select.up", []]] },
        { "do": "keys", "name": "p", "op": "reload" },
        { "do": "keys", "name": "p", "op": "getKeys", "id": "tui.select.up" },
        { "do": "keys", "name": "p", "op": "getUserBindings" },
        { "do": "keys", "name": "p", "op": "setUserBindings", "user": [["tui.select.up", null]] },
        { "do": "keys", "name": "p", "op": "reload" },
        { "do": "keys", "name": "p", "op": "getUserBindings" },
        { "do": "keys", "name": "c", "op": "getKeys", "id": "tui.select.up" },
    ]);
    let outcomes = script(runner, &steps).await;
    assert!(outcomes.iter().all(|outcome| outcome.get("ok").is_some()));
    assert_eq!(outcomes[2], ok(json!([["tui.select.up", "f7"]])));
    assert_eq!(outcomes[6], ok(json!([["tui.select.up", "f2"]])));
    for (index, expected) in [(3, "f7"), (4, "f7"), (7, "f2"), (8, "f2")] {
        let action = outcomes[index]["ok"]
            .as_array()
            .unwrap()
            .iter()
            .find(|binding| binding[0] == "tui.select.up")
            .unwrap();
        assert_eq!(action, &json!(["tui.select.up", expected]));
    }
    assert_eq!(outcomes[9], ok(json!(["f2"])));
    assert_eq!(outcomes[12], ok(json!([])));
    assert_eq!(outcomes[13], ok(json!([["tui.select.up", []]])));
    assert_eq!(outcomes[16], ok(json!([["tui.select.up", null]])));
    assert_eq!(outcomes[17], ok(json!(["f2"])));
}

#[test]
fn canonical_editors_reach_the_toolkit_editor_and_the_application_actions() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no diagnostics")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let ui = published_ui();
        runner.set_ui_context(Some(maestro_extensions::ExtensionUIContext::new(
            ui.clone(),
        )));
        let edit = |op: &str, key: &str, value: serde_json::Value| {
            let mut step = json!({ "do": "edit", "name": "e", "op": op });
            step[key] = value;
            step
        };
        let steps = json!([
            { "do": "editor", "name": "e", "theme": "authored", "options": [2, 7] },
            { "do": "on", "name": "e", "slot": "change", "label": "changed" },
            { "do": "on", "name": "e", "slot": "submit", "label": "sent" },
            edit("setText", "text", json!("hi")),
            { "do": "input", "name": "e", "data": "!" },
            edit("getLines", "text", json!(null)),
            edit("insertText", "text", json!("\nyo")),
            edit("getCursor", "text", json!(null)),
            { "do": "input", "name": "e", "data": "\r" },
            edit("getText", "text", json!(null)),
            edit("setPaddingX", "number", json!(3.7)),
            edit("getPaddingX", "text", json!(null)),
            edit("setAutocompleteMaxVisible", "number", json!(50)),
            edit("getAutocompleteMaxVisible", "text", json!(null)),
            edit("setDisableSubmit", "flag", json!(true)),
            edit("disableSubmit", "text", json!(null)),
            edit("setFocused", "flag", json!(true)),
            edit("focused", "text", json!(null)),
            edit("isShowingAutocomplete", "text", json!(null)),
            { "do": "render", "name": "e", "width": 8 },
            { "do": "views" },
        ]);
        let outcomes = script(&runner, &steps).await;
        let (done, unit) = (ok(json!(true)), ok(json!(null)));
        let rule = "border(─)".repeat(8);
        let mut expected = vec![done.clone(); 3];
        expected.extend([unit.clone(), done.clone(), ok(json!(["hi!"])), unit.clone()]);
        expected.extend([ok(json!([1, 2])), done, ok(json!("")), unit.clone()]);
        expected.extend([ok(json!(3.0)), unit.clone(), ok(json!(20.0)), unit.clone()]);
        expected.extend([ok(json!(true)), unit, ok(json!(true)), ok(json!(false))]);
        let cursor = "   \u{1b}_maestro:c\u{7}\u{1b}[7m \u{1b}[0m    ";
        expected.push(ok(json!([rule, cursor, rule])));
        let changes = ["changed hi", "changed hi!", "changed hi!\nyo", "changed "];
        let mut calls = changes.to_vec();
        calls.push("sent hi!\nyo");
        expected.push(ok(json!(calls)));
        assert_eq!(outcomes, expected);
        custom_editor(&runner).await;
        drop(runner);
        bus.clear();
    });
}

/// A custom editor offers input to its interceptor, its interrupt handler and its actions.
async fn custom_editor(runner: &ExtensionRunner<Local>) {
    let keys = json!([
        ["app.interrupt", "escape", "Interrupt"],
        ["app.demo", "ctrl+x", "Demo"]
    ]);
    let custom = json!([
        { "do": "keybindings", "name": "k", "source": "definitions", "definitions": keys },
        { "do": "editor", "name": "c", "keys": "k", "theme": "host" },
        { "do": "action", "name": "c", "action": "app.demo", "label": "demo" },
        { "do": "input", "name": "c", "data": "\u{18}" },
        { "do": "on", "name": "c", "slot": "escape", "label": "escape" },
        { "do": "input", "name": "c", "data": "\u{1b}" },
        { "do": "on", "name": "c", "slot": "shortcut", "label": "short" },
        { "do": "input", "name": "c", "data": "\u{b}" },
        { "do": "input", "name": "c", "data": "z" },
        { "do": "action", "name": "c", "action": "app.demo", "label": "demo2" },
        { "do": "input", "name": "c", "data": "\u{18}" },
        { "do": "edit", "name": "c", "op": "getText" },
        { "do": "views" },
    ]);
    let outcomes = script(runner, &custom).await;
    let mut expected = vec![ok(json!(true)); 11];
    expected.push(ok(json!("z")));
    let calls = [
        "demo",
        "escape",
        "short \u{b}",
        "short z",
        "short \u{18}",
        "demo2",
    ];
    expected.push(ok(json!(calls)));
    assert_eq!(
        outcomes, expected,
        "a consumed shortcut stops; a replaced action keeps its place"
    );
}

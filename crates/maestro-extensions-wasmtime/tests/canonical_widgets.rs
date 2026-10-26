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

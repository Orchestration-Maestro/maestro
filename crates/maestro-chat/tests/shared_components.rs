//! Public rendering, timer, input and inherited component witnesses.
#![cfg(test)]
use crate::manual_runtime;
use crate::published;
use crate::recording_terminal;
use crate::runtime_spy;
use maestro_chat::DynamicBorder;
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::{Component, TUI, TerminalImage};
use manual_runtime::{ManualRuntime, ms};
use recording_terminal::RecordingTerminal;
use runtime_spy::Spy;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// One recorded query with strict outer membership.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning witness.
    test: String,
    /// Typed by the owning witness.
    input: Value,
    /// Typed by the owning witness.
    expected: Value,
}
/// Decode only queries owned by this witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(I, E)> {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/shared_components.json")).unwrap();
    assert!(
        cases.iter().any(|case| case.test == name),
        "missing recorded witness"
    );
    cases
        .into_iter()
        .filter(|case| case.test == name)
        .map(|case| {
            (
                serde_json::from_value(case.input).unwrap(),
                serde_json::from_value(case.expected).unwrap(),
            )
        })
        .collect()
}
/// Prepared colors matching the recorded component themes.
fn colors(border: u8, accent: u8, muted: u8, dim: u8) -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [
                (ThemeColor::Border, ColorValue::Index(border)),
                (ThemeColor::BorderMuted, ColorValue::Index(border)),
                (ThemeColor::Accent, ColorValue::Index(accent)),
                (ThemeColor::Muted, ColorValue::Index(muted)),
                (ThemeColor::Dim, ColorValue::Index(dim)),
            ],
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// A real writer over the controlled terminal and runtime.
fn writer(runtime: &ManualRuntime) -> TUI {
    TUI::new(
        RecordingTerminal::new(80, 24).handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        Some(false),
    )
}
/// Typed query data for border clamps width and recolors each render.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderClampsWidthAndRecolorsEachRenderInput {
    /// Recorded width.
    width: usize,
    /// Recorded custom.
    custom: bool,
}

/// Typed query data for border clamps width and recolors each render.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderClampsWidthAndRecolorsEachRenderExpected {
    /// Recorded before.
    before: Vec<String>,
    /// Recorded after.
    after: Vec<String>,
    /// Recorded calls.
    calls: Option<usize>,
}

#[test]
fn border_clamps_width_and_recolors_each_render() {
    for (input, expected) in cases::<
        BorderClampsWidthAndRecolorsEachRenderInput,
        BorderClampsWidthAndRecolorsEachRenderExpected,
    >("border_clamps_width_and_recolors_each_render")
    {
        let (_state, live) = published::published(&colors(31, 32, 33, 34));
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        let custom = input.custom.then(|| {
            Rc::new(move |text: &str| {
                count.set(count.get() + 1);
                format!("[{}:{text}]", count.get())
            }) as maestro_tui::editor_component::BorderColor
        });
        let border = DynamicBorder::new(live, custom);
        assert_eq!(border.render(input.width), expected.before);
        border.invalidate();
        assert_eq!(border.render(input.width), expected.after);
        if let Some(expected) = expected.calls {
            assert_eq!(calls.get(), expected);
        }
    }
}

/// Typed query data for border uses live theme but custom color needs no theme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderUsesLiveThemeButCustomColorNeedsNoThemeInput {
    /// Recorded uninitialized.
    uninitialized: Option<bool>,
}

/// Typed query data for border uses live theme but custom color needs no theme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderUsesLiveThemeButCustomColorNeedsNoThemeExpected {
    /// Recorded before.
    before: Option<Vec<String>>,
    /// Recorded after.
    after: Option<Vec<String>>,
    /// Recorded result.
    result: Option<Vec<String>>,
}

#[test]
fn border_uses_live_theme_but_custom_color_needs_no_theme() {
    for (input, expected) in cases::<
        BorderUsesLiveThemeButCustomColorNeedsNoThemeInput,
        BorderUsesLiveThemeButCustomColorNeedsNoThemeExpected,
    >("border_uses_live_theme_but_custom_color_needs_no_theme")
    {
        if input.uninitialized.unwrap_or(false) {
            let state = published::unpublished();
            let border =
                DynamicBorder::new(state.theme(), Some(Rc::new(|text| format!("[{text}]"))));
            assert_eq!(Some(border.render(2)), expected.result);
        } else {
            let (state, live) = published::published(&colors(31, 32, 33, 34));
            let border = DynamicBorder::new(live, None);
            assert_eq!(Some(border.render(3)), expected.before);
            state.set_theme_instance(colors(35, 36, 37, 38)).unwrap();
            assert_eq!(Some(border.render(3)), expected.after);
        }
    }
}

#[test]
fn extension_widget_host_builds_widgets_over_the_live_theme_and_terminal() {
    use maestro_app::ExtensionWidgets;
    use maestro_tui::{Markdown, MarkdownOptions};
    let (state, live) = published::published(&colors(31, 32, 33, 34));
    let highlighter = Rc::new(maestro_theme::SyntectHighlighter::new().unwrap());
    let terminal = TerminalImage::new(|_| None, || 1);
    let host =
        maestro_chat::ExtensionWidgetHost::new(live.clone(), highlighter.clone(), terminal.clone());
    let border = host.dynamic_border(None);
    let application = DynamicBorder::new(live.clone(), None);
    assert_eq!(border.render(4), application.render(4));
    state.set_theme_instance(colors(35, 36, 37, 38)).unwrap();
    assert_eq!(
        border.render(4),
        application.render(4),
        "the border follows the live theme"
    );
    let painted = host.dynamic_border(Some(Rc::new(|text| format!("[{text}]"))));
    assert_eq!(painted.render(2), ["[──]"]);
    let options = || MarkdownOptions {
        clean_content: false,
        padding_x: 1,
        padding_y: 0,
        default_text_style: None,
    };
    let text = "# Title\n\n```rust\nfn main() {}\n```";
    let built = host.markdown(text.into(), options(), Rc::new(host.markdown_theme()));
    let theme = maestro_theme::get_markdown_theme(live, highlighter, true);
    let direct = Markdown::new(text.into(), options(), Rc::new(theme), terminal);
    assert_eq!(
        built.render(24),
        direct.render(24),
        "the toolkit Markdown with the live styles"
    );
}

#[test]
fn extension_widget_host_builds_application_loaders_and_list_styles() {
    use maestro_app::ExtensionWidgets;
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let highlighter = Rc::new(maestro_theme::SyntectHighlighter::new().unwrap());
    let host = maestro_chat::ExtensionWidgetHost::new(
        live.clone(),
        highlighter,
        TerminalImage::new(|_| None, || 1),
    );
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let theme = live.get().unwrap();
    for cancellable in [true, false] {
        let loader = host
            .bordered_loader(&tui, theme.clone(), "Working".into(), cancellable)
            .unwrap();
        let options = maestro_chat::BorderedLoaderOptions { cancellable };
        let direct = maestro_chat::BorderedLoader::new(
            &tui,
            theme.clone(),
            "Working".into(),
            options,
            &live,
        )
        .unwrap();
        assert_eq!(
            loader.render(30),
            direct.render(30),
            "the application's loader"
        );
        loader.dispose();
        direct.dispose();
    }
    let loader = host
        .bordered_loader(&tui, theme, "Working".into(), true)
        .unwrap();
    let aborted = Rc::new(Cell::new(false));
    let seen = aborted.clone();
    loader.set_on_abort(Some(Rc::new(move || seen.set(true))));
    loader.input_handler().unwrap().handle_input("\u{1b}");
    assert!(
        loader.signal().is_aborted() && aborted.get(),
        "escape cancels the loader"
    );
    assert_eq!(
        loader.rows().children().borrow().len(),
        6,
        "borders, loader, hint and spacers"
    );
    let select = host.select_list_theme();
    let direct = maestro_theme::get_select_list_theme(live.clone());
    assert_eq!((select.selected_text)("x"), (direct.selected_text)("x"));
    let settings = host.settings_list_theme();
    let direct = maestro_theme::get_settings_list_theme(live);
    assert_eq!((settings.value)("x", false), (direct.value)("x", false));
    assert_eq!(settings.cursor, direct.cursor);
}

#[test]
fn extension_widget_host_lends_application_keybindings_hints_and_editor_styles() {
    use maestro_app::ExtensionWidgets;
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let highlighter = Rc::new(maestro_theme::SyntectHighlighter::new().unwrap());
    let host = maestro_chat::ExtensionWidgetHost::new(
        live.clone(),
        highlighter,
        TerminalImage::new(|_| None, || 1),
    );
    assert_application_keybindings(&host);
    let hint = host.key_hint("tui.select.up", "up").unwrap();
    assert_eq!(
        hint,
        maestro_app::key_hint("tui.select.up", "up", &live).unwrap()
    );
    let raw = host.raw_key_hint("esc", "back").unwrap();
    assert_eq!(
        raw,
        maestro_app::raw_key_hint("esc", "back", &live).unwrap()
    );
    let theme = host.editor_theme();
    let direct = maestro_theme::get_editor_theme(live);
    assert_eq!((theme.border_color)("x"), (direct.border_color)("x"));
}

/// The supplied managers retain their own file, overrides and effective snapshots.
fn assert_application_keybindings(host: &dyn maestro_app::ExtensionWidgets) {
    use maestro_tui::KeybindingKeys;
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let dir = scratch.path().to_str().unwrap().to_owned();
    let user = vec![(
        "tui.select.up".to_owned(),
        Some(KeybindingKeys::Single("f3".to_owned())),
    )];
    let keybindings = host.keybindings(user.clone(), None);
    assert_eq!(keybindings.get_user_bindings(), user);
    assert_eq!(keybindings.get_keys("tui.select.up"), ["f3"]);
    keybindings.reload();
    assert_eq!(
        keybindings.get_user_bindings(),
        user,
        "no file keeps the bindings"
    );
    let created = host.create_keybindings(Some(dir.clone())).unwrap();
    assert_eq!(created.get_keys("tui.select.up"), ["up"]);
    let retained = created.clone();
    let mut snapshot = retained.get_effective_config();
    snapshot.clear();
    let path = format!("{dir}/keybindings.json");
    std::fs::write(&path, r#"{"tui.select.up":"f9"}"#).unwrap();
    drop(created);
    retained.reload();
    assert_eq!(
        retained.get_keys("tui.select.up"),
        ["f9"],
        "reloads its file"
    );
    assert!(retained.get_effective_config().contains(&(
        "tui.select.up".into(),
        Some(KeybindingKeys::Single("f9".into()))
    )));
    let explicit = host.keybindings(user, Some(path.clone()));
    assert_eq!(
        explicit.get_keys("tui.select.up"),
        ["f3"],
        "constructor is inert"
    );
    explicit.reload();
    assert_eq!(explicit.get_keys("tui.select.up"), ["f9"]);
    std::fs::write(path, r#"{"tui.select.up":[]}"#).unwrap();
    explicit.reload();
    assert!(explicit.get_keys("tui.select.up").is_empty());
    assert_eq!(
        retained.get_keys("tui.select.up"),
        ["f9"],
        "independent manager"
    );
    assert_eq!(
        keybindings.get_keys("tui.select.up"),
        ["f3"],
        "pathless manager"
    );
}

#[test]
fn extension_widget_host_builds_the_application_custom_editor() {
    use maestro_app::ExtensionWidgets;
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let highlighter = Rc::new(maestro_theme::SyntectHighlighter::new().unwrap());
    let host = maestro_chat::ExtensionWidgetHost::new(
        live,
        highlighter,
        TerminalImage::new(|_| None, || 1),
    );
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let keybindings = host.keybindings(Vec::new(), None).manager;
    let options = maestro_tui::EditorOptions {
        padding_x: Some(2.0),
        autocomplete_max_visible: None,
    };
    let editor = host.custom_editor(&tui, host.editor_theme(), keybindings, options);
    assert_eq!(
        editor.editor().get_padding_x().to_bits(),
        2.0_f64.to_bits(),
        "the options reach the editor"
    );
    let interrupted = Rc::new(Cell::new(0));
    let seen = interrupted.clone();
    let handler: Rc<dyn Fn()> = Rc::new(move || seen.set(seen.get() + 1));
    editor
        .action_handlers()
        .borrow_mut()
        .insert("app.interrupt".to_owned(), handler);
    editor.input_handler().unwrap().handle_input("\u{1b}");
    assert_eq!(
        interrupted.get(),
        1,
        "the application's interrupt action runs"
    );
    editor.input_handler().unwrap().handle_input("x");
    assert_eq!(editor.get_text(), "x", "other input edits");
}

#[test]
fn border_returns_painter_output_without_preserving_rule_glyphs() {
    let state = published::unpublished();
    let border = DynamicBorder::new(state.theme(), Some(Rc::new(|text| text.replace('─', "="))));
    assert_eq!(border.render(0), ["="]);
    assert_eq!(border.render(3), ["==="]);
}

/// Writer over the scheduling observer.
fn spy_writer(spy: &Spy) -> TUI {
    TUI::new(
        RecordingTerminal::new(80, 24).handle(),
        Rc::new(spy.clone()),
        TerminalImage::new(|_| None, || 1),
        Some(false),
    )
}
/// Typed query data for countdown ceil tick render expire order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CountdownCeilTickRenderExpireOrderInput {
    /// Recorded timeout ms.
    timeout_ms: f64,
    /// Recorded has tui.
    has_tui: bool,
}

/// Typed query data for countdown ceil tick render expire order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountdownCeilTickRenderExpireOrderExpected {
    /// Recorded initial.
    initial: Vec<CountEvent>,
    /// Recorded delays.
    delays: Vec<u64>,
    /// Recorded events.
    events: Vec<CountEvent>,
    /// Recorded pending.
    pending: usize,
}

#[test]
fn countdown_ceil_tick_render_expire_order() {
    use maestro_chat::CountdownTimer;
    for (input, expected) in cases::<
        CountdownCeilTickRenderExpireOrderInput,
        CountdownCeilTickRenderExpireOrderExpected,
    >("countdown_ceil_tick_render_expire_order")
    {
        let spy = Spy::default();
        let tui = spy_writer(&spy);
        let events = spy.events.clone();
        let expired = spy.clone();
        let timer = CountdownTimer::new(
            input.timeout_ms,
            input.has_tui.then_some(&tui),
            Rc::new(spy.clone()),
            Rc::new(move |n| events.borrow_mut().push(serde_json::json!(["tick", n]))),
            Rc::new(move || {
                expired
                    .events
                    .borrow_mut()
                    .push(serde_json::json!(["expire", expired.pending.get()]));
            }),
        );
        assert_eq!(count_events(&spy), expected.initial);
        assert_eq!(*spy.delays.borrow(), expected.delays);
        assert_eq!(spy.clock.next_deadline(), Some(ms(1000)));
        spy.clock
            .advance_to(ms(1000))
            .into_iter()
            .for_each(Result::unwrap);
        spy.clock
            .advance_to(ms(2000))
            .into_iter()
            .for_each(Result::unwrap);
        assert_eq!(count_events(&spy), expected.events);
        assert_eq!(spy.pending.get(), expected.pending);
        timer.dispose();
    }
}

/// Ordered countdown observations, with numeric values decoded as numbers.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum CountEvent {
    /// Notification or expiry.
    Number((String, f64)),
    /// Render request.
    Word((String,)),
}
/// Decode the spy transcript without losing sequence or payload.
fn count_events(spy: &Spy) -> Vec<CountEvent> {
    serde_json::from_value(Value::Array(spy.events.borrow().clone())).unwrap()
}

/// Typed query data for countdown disposal is idempotent and reentrant.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountdownDisposalIsIdempotentAndReentrantInput {
    /// Recorded mode.
    mode: String,
}

/// Typed query data for countdown disposal is idempotent and reentrant.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountdownDisposalIsIdempotentAndReentrantExpected {
    /// Recorded events.
    events: Vec<Value>,
    /// Recorded pending.
    pending: usize,
}

#[test]
fn countdown_disposal_is_idempotent_and_reentrant() {
    for (input, expected) in cases::<
        CountdownDisposalIsIdempotentAndReentrantInput,
        CountdownDisposalIsIdempotentAndReentrantExpected,
    >("countdown_disposal_is_idempotent_and_reentrant")
    {
        let spy = Spy::default();
        let tui = spy_writer(&spy);
        let slot = Rc::new(RefCell::new(None::<maestro_chat::CountdownTimer>));
        let tick_slot = slot.clone();
        let tick_spy = spy.clone();
        let mode = input.mode.clone();
        let expire_slot = slot.clone();
        let expired = spy.clone();
        let timer = maestro_chat::CountdownTimer::new(
            2000.0,
            Some(&tui),
            Rc::new(spy.clone()),
            Rc::new(move |n| {
                tick_spy
                    .events
                    .borrow_mut()
                    .push(serde_json::json!(["tick", n]));
                if (mode == "in_tick" && n.to_bits() == 1.0_f64.to_bits())
                    || (mode == "in_terminal_tick" && n.to_bits() == 0.0_f64.to_bits())
                {
                    tick_slot.borrow().as_ref().unwrap().dispose();
                }
            }),
            Rc::new(move || {
                assert_eq!(expired.pending.get(), 0);
                expired
                    .events
                    .borrow_mut()
                    .push(serde_json::json!(["expire"]));
                expire_slot.borrow().as_ref().unwrap().dispose();
            }),
        );
        slot.replace(Some(timer));
        if input.mode == "before" {
            slot.borrow().as_ref().unwrap().dispose();
            slot.borrow().as_ref().unwrap().dispose();
        }
        for time in [1000, 2000, 3000] {
            spy.clock
                .advance_to(ms(time))
                .into_iter()
                .for_each(Result::unwrap);
        }
        let observed = disposal_events(&spy);
        assert_eq!(observed, expected.events);
        assert_eq!(spy.pending.get(), expected.pending);
        slot.take();
    }
}
#[test]
fn countdown_drop_releases_callbacks_and_pending_tick() {
    let runtime = ManualRuntime::new();
    let capture = Rc::new(Cell::new(0));
    let weak = Rc::downgrade(&capture);
    let timer = maestro_chat::CountdownTimer::new(
        2000.0,
        None,
        runtime.handle(),
        Rc::new(move |_| capture.set(capture.get() + 1)),
        Rc::new(|| panic!("disposed countdown expired")),
    );
    timer.dispose();
    assert_eq!(runtime.pending(), 0);
    assert_eq!(weak.upgrade().unwrap().get(), 1);
    drop(timer);
    assert!(weak.upgrade().is_none());
    let capture = Rc::new(Cell::new(0));
    let weak = Rc::downgrade(&capture);
    let timer = maestro_chat::CountdownTimer::new(
        2000.0,
        None,
        runtime.handle(),
        Rc::new(move |_| capture.set(capture.get() + 1)),
        Rc::new(|| panic!("dropped countdown expired")),
    );
    assert_eq!(runtime.pending(), 1);
    drop(timer);
    assert!(weak.upgrade().is_none());
    assert_eq!(runtime.pending(), 0);
    assert!(runtime.advance_to(ms(3000)).is_empty());
}
/// A writer-owned component retaining countdown policy.
struct CountdownComponent {
    /// Countdown dropped with the component.
    _timer: maestro_chat::CountdownTimer,
}
impl Component for CountdownComponent {
    fn render(&self, _width: usize) -> Vec<String> {
        vec![]
    }
}
#[test]
fn countdown_containing_writer_drop_releases_component_callbacks_and_pending_tick() {
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let capture = Rc::new(Cell::new(0));
    let weak_capture = Rc::downgrade(&capture);
    let component = Rc::new(CountdownComponent {
        _timer: maestro_chat::CountdownTimer::new(
            2000.0,
            Some(&tui),
            runtime.handle(),
            Rc::new(move |_| capture.set(capture.get() + 1)),
            Rc::new(|| panic!("dropped component countdown expired")),
        ),
    });
    let weak_component = Rc::downgrade(&component);
    tui.add_child(component.clone());
    assert_eq!(runtime.pending(), 1);
    assert_eq!(weak_capture.upgrade().unwrap().get(), 1);
    drop(component);
    drop(tui);
    assert!(
        weak_component.upgrade().is_none(),
        "writer retained component"
    );
    assert!(
        weak_capture.upgrade().is_none(),
        "callback retained capture"
    );
    assert_eq!(runtime.pending(), 0);
    assert!(runtime.advance_to(ms(3000)).is_empty());
}
/// Retain toolkit defaults while registering explicit application actions.
fn bindings(actions: &[(&str, Vec<String>)]) -> maestro_tui::KeybindingsManager {
    let mut definitions = maestro_tui::TUI_KEYBINDINGS.clone();
    definitions.extend(actions.iter().map(|(action, keys)| {
        (
            action.to_string(),
            maestro_tui::KeybindingDefinition {
                default_keys: maestro_tui::KeybindingKeys::Multiple(keys.clone()),
                description: None,
            },
        )
    }));
    maestro_tui::KeybindingsManager::new(definitions, vec![])
}
/// Typed query data for bordered loader layout preserves padding and message.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderedLoaderLayoutPreservesPaddingAndMessageInput {
    /// Recorded cancellable.
    cancellable: Option<bool>,
    /// Recorded width.
    width: usize,
    /// Recorded message.
    message: String,
}

/// Typed query data for bordered loader layout preserves padding and message.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderedLoaderLayoutPreservesPaddingAndMessageExpected {
    /// Recorded first.
    first: Vec<String>,
    /// Recorded next.
    next: Vec<String>,
}

#[test]
fn bordered_loader_layout_preserves_padding_and_message() {
    let _guard = input_support::globals();
    maestro_tui::set_keybindings(bindings(&[(
        "tui.select.cancel",
        vec!["escape".into(), "ctrl+c".into()],
    )]));
    for (input, expected) in cases::<
        BorderedLoaderLayoutPreservesPaddingAndMessageInput,
        BorderedLoaderLayoutPreservesPaddingAndMessageExpected,
    >("bordered_loader_layout_preserves_padding_and_message")
    {
        let runtime = ManualRuntime::new();
        let tui = writer(&runtime);
        let (_state, live) = published::published(&colors(31, 32, 33, 34));
        let options = input.cancellable.map_or_else(
            maestro_chat::BorderedLoaderOptions::default,
            |cancellable| maestro_chat::BorderedLoaderOptions { cancellable },
        );
        let loader = maestro_chat::BorderedLoader::new(
            &tui,
            colors(32, 33, 34, 35),
            input.message,
            options,
            &live,
        )
        .unwrap();
        assert_eq!(loader.render(input.width), expected.first);
        runtime
            .advance_to(ms(80))
            .into_iter()
            .for_each(Result::unwrap);
        assert_eq!(loader.render(input.width), expected.next);
        loader.dispose();
        runtime.settle().unwrap();
    }
}

use crate::input_support;
/// Typed query data for bordered loader keeps captured theme and hint but matches live keys.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BorderedLoaderKeepsCapturedThemeAndHintButMatchesLiveKeysExpected {
    /// Recorded before.
    before: Vec<String>,
    /// Recorded after.
    after: Vec<String>,
    /// Recorded old.
    old: bool,
    /// Recorded aborted.
    aborted: bool,
    /// Recorded seen.
    seen: Vec<bool>,
    /// Recorded pending.
    pending: usize,
    /// Recorded same signal.
    same_signal: bool,
}

#[test]
fn bordered_loader_keeps_captured_theme_and_hint_but_matches_live_keys() {
    let _guard = input_support::globals();
    let [(_, expected)] =
        cases::<EmptyInput, BorderedLoaderKeepsCapturedThemeAndHintButMatchesLiveKeysExpected>(
            "bordered_loader_keeps_captured_theme_and_hint_but_matches_live_keys",
        )
        .try_into()
        .ok()
        .unwrap();
    let manager = bindings(&[("tui.select.cancel", vec!["escape".into(), "ctrl+c".into()])]);
    maestro_tui::set_keybindings(manager.clone());
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let (state, live) = published::published(&colors(31, 32, 33, 34));
    let loader = maestro_chat::BorderedLoader::new(
        &tui,
        colors(32, 33, 34, 35),
        "Working".into(),
        maestro_chat::BorderedLoaderOptions::default(),
        &live,
    )
    .unwrap();
    assert_eq!(loader.render(30), expected.before);
    state.set_theme_instance(colors(35, 36, 37, 38)).unwrap();
    manager.set_user_bindings(vec![(
        "tui.select.cancel".into(),
        Some(maestro_tui::KeybindingKeys::Single("ctrl+c".into())),
    )]);
    loader.invalidate();
    assert_eq!(loader.render(30), expected.after);
    let signal = loader.signal().clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let observed = seen.clone();
    let aborted = signal.clone();
    loader.set_on_abort(Some(Rc::new(move || {
        observed.borrow_mut().push(aborted.is_aborted());
    })));
    loader.input_handler().unwrap().handle_input("\x1b");
    assert_eq!(signal.is_aborted(), expected.old);
    for _ in 0..2 {
        loader.input_handler().unwrap().handle_input("\x03");
    }
    loader.set_on_abort(None);
    loader.input_handler().unwrap().handle_input("\x03");
    loader.dispose();
    runtime.settle().unwrap();
    assert_eq!(signal.is_aborted(), expected.aborted);
    assert_eq!(*seen.borrow(), expected.seen);
    assert_eq!(runtime.pending(), expected.pending);
    assert_eq!(
        std::ptr::eq(loader.signal(), loader.signal()),
        expected.same_signal
    );
}
/// Typed query data for bordered loader dispose stops animation without forcing abort.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderedLoaderDisposeStopsAnimationWithoutForcingAbortInput {
    /// Recorded cancellable.
    cancellable: bool,
}

/// Typed query data for bordered loader dispose stops animation without forcing abort.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BorderedLoaderDisposeStopsAnimationWithoutForcingAbortExpected {
    /// Recorded unmatched.
    unmatched: bool,
    /// Recorded disposed.
    disposed: bool,
    /// Recorded called.
    called: usize,
    /// Recorded aborted.
    aborted: bool,
    /// Recorded pending.
    pending: usize,
}

#[test]
fn bordered_loader_dispose_stops_animation_without_forcing_abort() {
    let _guard = input_support::globals();
    maestro_tui::set_keybindings(bindings(&[]));
    for (input, expected) in cases::<
        BorderedLoaderDisposeStopsAnimationWithoutForcingAbortInput,
        BorderedLoaderDisposeStopsAnimationWithoutForcingAbortExpected,
    >("bordered_loader_dispose_stops_animation_without_forcing_abort")
    {
        let runtime = ManualRuntime::new();
        let tui = writer(&runtime);
        let (_state, live) = published::published(&colors(31, 32, 33, 34));
        let loader = maestro_chat::BorderedLoader::new(
            &tui,
            colors(31, 32, 33, 34),
            String::new(),
            maestro_chat::BorderedLoaderOptions {
                cancellable: input.cancellable,
            },
            &live,
        )
        .unwrap();
        let called = Rc::new(Cell::new(0));
        let calls = called.clone();
        loader.set_on_abort(Some(Rc::new(move || calls.set(calls.get() + 1))));
        loader.input_handler().unwrap().handle_input("z");
        assert_eq!(loader.signal().is_aborted(), expected.unmatched);
        loader.dispose();
        loader.dispose();
        assert_eq!(loader.signal().is_aborted(), expected.disposed);
        let stopped = loader.render(12);
        runtime.settle().unwrap();
        assert_eq!(loader.render(12), stopped);
        for _ in 0..2 {
            loader.input_handler().unwrap().handle_input("\x1b");
        }
        assert_eq!(called.get(), expected.called);
        assert_eq!(loader.signal().is_aborted(), expected.aborted);
        assert_eq!(runtime.pending(), expected.pending);
    }
}
/// Typed query data for plain loader never aborts or reads hint theme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PlainLoaderNeverAbortsOrReadsHintThemeExpected {
    /// Recorded called.
    called: usize,
    /// Recorded aborted.
    aborted: bool,
    /// Recorded same signal.
    same_signal: bool,
    /// Recorded pending.
    pending: usize,
}

#[test]
fn plain_loader_never_aborts_or_reads_hint_theme() {
    let [(_, expected)] = cases::<EmptyInput, PlainLoaderNeverAbortsOrReadsHintThemeExpected>(
        "plain_loader_never_aborts_or_reads_hint_theme",
    )
    .try_into()
    .ok()
    .unwrap();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let state = published::unpublished();
    let loader = maestro_chat::BorderedLoader::new(
        &tui,
        colors(31, 32, 33, 34),
        "Plain".into(),
        maestro_chat::BorderedLoaderOptions { cancellable: false },
        &state.theme(),
    )
    .unwrap();
    let called = Rc::new(Cell::new(0));
    let calls = called.clone();
    loader.set_on_abort(Some(Rc::new(move || calls.set(calls.get() + 1))));
    let signal = loader.signal();
    for data in ["\x1b", "\x03"] {
        loader.input_handler().unwrap().handle_input(data);
    }
    loader.dispose();
    loader.dispose();
    runtime.settle().unwrap();
    assert_eq!(called.get(), expected.called);
    assert_eq!(signal.is_aborted(), expected.aborted);
    assert_eq!(std::ptr::eq(signal, loader.signal()), expected.same_signal);
    assert_eq!(runtime.pending(), expected.pending);
    assert!(
        maestro_chat::BorderedLoader::new(
            &tui,
            colors(31, 32, 33, 34),
            "error".into(),
            maestro_chat::BorderedLoaderOptions::default(),
            &state.theme()
        )
        .is_err()
    );
    runtime.settle().unwrap();
}
/// Typed query data for application hint exports use owner semantics.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationHintExportsUseOwnerSemanticsInput {
    /// Recorded keys.
    keys: Vec<String>,
}

/// Typed query data for application hint exports use owner semantics.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationHintExportsUseOwnerSemanticsExpected {
    /// Recorded text.
    text: String,
    /// Recorded hint.
    hint: String,
    /// Recorded raw.
    raw: String,
}

#[test]
fn application_hint_exports_use_owner_semantics() {
    let _guard = input_support::globals();
    for (input, expected) in cases::<
        ApplicationHintExportsUseOwnerSemanticsInput,
        ApplicationHintExportsUseOwnerSemanticsExpected,
    >("application_hint_exports_use_owner_semantics")
    {
        maestro_tui::set_keybindings(bindings(&[("tui.select.cancel", input.keys)]));
        let (_state, live) = published::published(&colors(31, 32, 33, 34));
        assert_eq!(maestro_app::key_text("tui.select.cancel"), expected.text);
        assert_eq!(
            maestro_app::key_hint("tui.select.cancel", "cancel", &live).unwrap(),
            expected.hint
        );
        assert_eq!(
            maestro_app::raw_key_hint("Z", "cancel", &live).unwrap(),
            expected.raw
        );
    }
}
/// Typed query data for loader container mutations keep shared child identity.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LoaderContainerMutationsKeepSharedChildIdentityExpected {
    /// Recorded added.
    added: String,
    /// Recorded after clear.
    after_clear: Vec<String>,
    /// Recorded old length.
    old_length: usize,
    /// Recorded restored.
    restored: Vec<String>,
    /// Recorded final length.
    final_length: usize,
}

#[test]
fn loader_container_mutations_keep_shared_child_identity() {
    let _guard = input_support::globals();
    maestro_tui::set_keybindings(bindings(&[]));
    let [(_, expected)] = cases::<
        EmptyInput,
        LoaderContainerMutationsKeepSharedChildIdentityExpected,
    >("loader_container_mutations_keep_shared_child_identity")
    .try_into()
    .ok()
    .unwrap();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let loader = maestro_chat::BorderedLoader::new(
        &tui,
        colors(31, 32, 33, 34),
        "Working".into(),
        maestro_chat::BorderedLoaderOptions::default(),
        &live,
    )
    .unwrap();
    let alias = loader.children();
    let child: maestro_tui::tui::ComponentHandle = Rc::new(Tail);
    loader.add_child(child.clone());
    assert_eq!(loader.render(30).last(), Some(&expected.added));
    loader.clear();
    alias.borrow_mut().push(child.clone());
    assert_eq!(loader.render(30), expected.after_clear);
    assert_eq!(alias.borrow().len(), expected.old_length);
    loader.set_children(alias);
    let render = loader.render(30);
    assert_eq!(&render[render.len() - 2..], expected.restored);
    loader.remove_child(&child);
    assert_eq!(loader.children().borrow().len(), expected.final_length);
    loader.dispose();
    runtime.settle().unwrap();
}

/// Construct an editor with recorded constructor options.
fn editor(
    tui: &TUI,
    live: maestro_theme::LiveTheme,
    manager: maestro_tui::KeybindingsManager,
) -> maestro_chat::CustomEditor {
    maestro_chat::CustomEditor::new(
        tui,
        maestro_theme::get_editor_theme(live),
        manager,
        maestro_tui::EditorOptions {
            padding_x: Some(2.0),
            autocomplete_max_visible: Some(7.0),
        },
    )
}
/// Application definitions used by the recorded routing queries.
fn app_bindings() -> maestro_tui::KeybindingsManager {
    bindings(&[
        ("app.clipboard.pasteImage", vec!["ctrl+v".into()]),
        ("app.interrupt", vec!["escape".into()]),
        ("app.exit", vec!["ctrl+d".into()]),
        ("app.first", vec!["ctrl+x".into()]),
        ("app.second", vec!["ctrl+x".into()]),
    ])
}
/// A callback appending its payload to the shared transcript.
fn callback(events: &Rc<RefCell<Vec<Value>>>, label: &str) -> Rc<dyn Fn()> {
    let events = events.clone();
    let label = label.to_owned();
    Rc::new(move || events.borrow_mut().push(Value::String(label.clone())))
}
/// Typed query data for custom editor routes actions in source order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomEditorRoutesActionsInSourceOrderInput {
    /// Recorded name.
    name: String,
    /// Recorded data.
    data: String,
    /// Recorded extension.
    extension: Option<bool>,
    /// Recorded paste.
    paste: Option<String>,
    /// Recorded interrupt.
    interrupt: Option<String>,
    /// Recorded exit.
    exit: Option<String>,
    /// Recorded generic.
    generic: Option<String>,
    /// Recorded dynamic.
    dynamic: Option<bool>,
    /// Recorded registered.
    registered: Option<bool>,
    /// Recorded text.
    text: Option<String>,
    /// Recorded home.
    home: Option<bool>,
}

/// Typed query data for custom editor routes actions in source order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomEditorRoutesActionsInSourceOrderCursor {
    /// Recorded line.
    line: usize,
    /// Recorded col.
    col: usize,
}

/// Typed query data for custom editor routes actions in source order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomEditorRoutesActionsInSourceOrderExpected {
    /// Recorded events.
    events: Vec<Value>,
    /// Recorded text.
    text: String,
    /// Recorded cursor.
    cursor: CustomEditorRoutesActionsInSourceOrderCursor,
}

#[test]
fn custom_editor_routes_actions_in_source_order() {
    let _guard = input_support::globals();
    for (input, expected) in cases::<
        CustomEditorRoutesActionsInSourceOrderInput,
        CustomEditorRoutesActionsInSourceOrderExpected,
    >("custom_editor_routes_actions_in_source_order")
    {
        let runtime = ManualRuntime::new();
        let tui = writer(&runtime);
        let (_state, live) = published::published(&colors(31, 32, 33, 34));
        let manager = app_bindings();
        maestro_tui::set_keybindings(manager.clone());
        let editor = editor(&tui, live, manager.clone());
        let events = Rc::new(RefCell::new(Vec::new()));
        configure_routing(&editor, &manager, &input, &events);
        editor.input_handler().unwrap().handle_input(&input.data);
        assert_eq!(*events.borrow(), expected.events, "{}", input.name);
        assert_eq!(editor.get_text(), expected.text, "{}", input.name);
        assert_eq!(
            editor.get_cursor().line,
            expected.cursor.line,
            "{}",
            input.name
        );
        assert_eq!(
            editor.get_cursor().col,
            expected.cursor.col,
            "{}",
            input.name
        );
        runtime.settle().unwrap();
    }
}

/// Typed query data for custom editor preserves order and replaceable map aliases.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CustomEditorPreservesOrderAndReplaceableMapAliasesExpected {
    /// Recorded keys.
    keys: Vec<String>,
    /// Recorded alias keys.
    alias_keys: Vec<String>,
    /// Recorded current keys.
    current_keys: Vec<String>,
    /// Recorded events.
    events: Vec<Value>,
    /// Recorded text.
    text: String,
}

#[test]
fn custom_editor_preserves_order_and_replaceable_map_aliases() {
    let _guard = input_support::globals();
    let [(_, expected)] = cases::<
        EmptyInput,
        CustomEditorPreservesOrderAndReplaceableMapAliasesExpected,
    >("custom_editor_preserves_order_and_replaceable_map_aliases")
    .try_into()
    .ok()
    .unwrap();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let manager = app_bindings();
    manager.set_user_bindings(vec![
        (
            "app.first".into(),
            Some(maestro_tui::KeybindingKeys::Single("x".into())),
        ),
        (
            "app.second".into(),
            Some(maestro_tui::KeybindingKeys::Single("x".into())),
        ),
    ]);
    let editor = editor(&tui, live, manager);
    let events = Rc::new(RefCell::new(Vec::new()));
    let first = callback(&events, "old");
    editor.on_action("app.first".into(), first.clone());
    editor.on_action("app.second".into(), callback(&events, "second"));
    let alias = editor.action_handlers();
    let map = Rc::downgrade(&alias);
    let log = events.clone();
    editor.on_action(
        "app.first".into(),
        Rc::new(move || {
            log.borrow_mut().push(serde_json::json!("replacement"));
            map.upgrade()
                .unwrap()
                .borrow_mut()
                .shift_remove("app.first");
        }),
    );
    assert_eq!(map_keys(&alias), expected.keys);
    editor.input_handler().unwrap().handle_input("x");
    editor.input_handler().unwrap().handle_input("x");
    let new = Rc::new(RefCell::new(indexmap::IndexMap::new()));
    new.borrow_mut()
        .insert("app.first".into(), callback(&events, "new_map"));
    editor.set_action_handlers(new);
    alias.borrow_mut().insert("app.first".into(), first);
    editor.input_handler().unwrap().handle_input("x");
    assert_eq!(map_keys(&alias), expected.alias_keys);
    assert_eq!(map_keys(&editor.action_handlers()), expected.current_keys);
    assert_eq!(*events.borrow(), expected.events);
    assert_eq!(editor.get_text(), expected.text);
    reverse_lexical_handler_order(&editor, &events);
}
/// Typed query data for custom editor callback replacement and reentry take effect.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomEditorCallbackReplacementAndReentryTakeEffectExpected {
    /// Recorded events.
    events: Vec<Value>,
    /// Recorded padding.
    padding: f64,
    /// Recorded maximum.
    maximum: usize,
    /// Recorded text.
    text: String,
}

#[test]
fn custom_editor_callback_replacement_and_reentry_take_effect() {
    let _guard = input_support::globals();
    let [(_, expected)] = cases::<
        EmptyInput,
        CustomEditorCallbackReplacementAndReentryTakeEffectExpected,
    >("custom_editor_callback_replacement_and_reentry_take_effect")
    .try_into()
    .ok()
    .unwrap();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let manager = app_bindings();
    let editor = Rc::new(editor(&tui, live, manager.clone()));
    let events = Rc::new(RefCell::new(Vec::new()));
    editor.set_on_escape(Some(callback(&events, "first")));
    editor.input_handler().unwrap().handle_input("\x1b");
    editor.set_on_escape(Some(callback(&events, "second")));
    editor.input_handler().unwrap().handle_input("\x1b");
    editor.set_on_escape(None);
    editor.on_action("app.interrupt".into(), callback(&events, "registered"));
    editor.input_handler().unwrap().handle_input("\x1b");
    let weak = Rc::downgrade(&editor);
    let added = callback(&events, "added");
    editor.set_on_extension_shortcut(Some(Rc::new(move |_| {
        weak.upgrade()
            .unwrap()
            .on_action("app.first".into(), added.clone());
        false
    })));
    manager.set_user_bindings(vec![(
        "app.first".into(),
        Some(maestro_tui::KeybindingKeys::Single("x".into())),
    )]);
    editor.input_handler().unwrap().handle_input("x");
    assert_eq!(*events.borrow(), expected.events);
    assert_eq!(editor.get_padding_x().to_bits(), expected.padding.to_bits());
    assert_eq!(editor.get_autocomplete_max_visible(), expected.maximum);
    assert_eq!(editor.get_text(), expected.text);
}
/// Typed query data for custom editor uses supplied manager after global replacement.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomEditorUsesSuppliedManagerAfterGlobalReplacementExpected {
    /// Recorded events.
    events: Vec<Value>,
    /// Recorded text.
    text: String,
}

#[test]
fn custom_editor_uses_supplied_manager_after_global_replacement() {
    let _guard = input_support::globals();
    let [(_, expected)] = cases::<
        EmptyInput,
        CustomEditorUsesSuppliedManagerAfterGlobalReplacementExpected,
    >("custom_editor_uses_supplied_manager_after_global_replacement")
    .try_into()
    .ok()
    .unwrap();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let manager = app_bindings();
    let editor = editor(&tui, live, manager.clone());
    let events = Rc::new(RefCell::new(Vec::new()));
    let other = app_bindings();
    other.set_user_bindings(vec![(
        "app.first".into(),
        Some(maestro_tui::KeybindingKeys::Single("y".into())),
    )]);
    maestro_tui::set_keybindings(other);
    manager.set_user_bindings(vec![(
        "app.first".into(),
        Some(maestro_tui::KeybindingKeys::Single("x".into())),
    )]);
    editor.on_action("app.first".into(), callback(&events, "first"));
    for data in ["x", "y"] {
        editor.input_handler().unwrap().handle_input(data);
    }
    manager.set_user_bindings(vec![(
        "app.first".into(),
        Some(maestro_tui::KeybindingKeys::Single("z".into())),
    )]);
    for data in ["x", "z"] {
        editor.input_handler().unwrap().handle_input(data);
    }
    assert_eq!(*events.borrow(), expected.events);
    assert_eq!(editor.get_text(), expected.text);
    runtime.settle().unwrap();
}
/// A controlled public provider returning two candidates immediately when polled.
struct Completion;
impl maestro_tui::AutocompleteProvider for Completion {
    type Signal = maestro_tui::Cancellation;
    fn get_suggestions<'a>(
        &'a self,
        _lines: &'a [String],
        _cursor: maestro_tui::autocomplete::CursorPosition,
        _options: maestro_tui::autocomplete::CompletionOptions<'a, Self::Signal>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        Option<maestro_tui::AutocompleteSuggestions>,
                        maestro_tui::autocomplete::CompletionError,
                    >,
                > + 'a,
        >,
    > {
        Box::pin(async {
            Ok(Some(maestro_tui::AutocompleteSuggestions {
                items: ["one", "two"]
                    .map(|s| maestro_tui::AutocompleteItem {
                        value: s.into(),
                        label: s.into(),
                        description: None,
                    })
                    .to_vec(),
                prefix: String::new(),
            }))
        })
    }
    fn apply_completion(
        &self,
        _lines: &[String],
        _cursor: maestro_tui::autocomplete::CursorPosition,
        item: &maestro_tui::AutocompleteItem,
        _prefix: &str,
    ) -> maestro_tui::autocomplete::CompletionResult {
        maestro_tui::autocomplete::CompletionResult {
            lines: vec![item.value.clone()],
            cursor_line: 0,
            cursor_col: item.value.len(),
        }
    }
}
/// Typed query data for custom editor autocomplete owns first interrupt.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomEditorAutocompleteOwnsFirstInterruptExpected {
    /// Recorded before.
    before: bool,
    /// Recorded after.
    after: bool,
    /// Recorded events.
    events: Vec<Value>,
    /// Recorded text.
    text: String,
}

#[test]
fn custom_editor_autocomplete_owns_first_interrupt() {
    let _guard = input_support::globals();
    let [(_, expected)] = cases::<EmptyInput, CustomEditorAutocompleteOwnsFirstInterruptExpected>(
        "custom_editor_autocomplete_owns_first_interrupt",
    )
    .try_into()
    .ok()
    .unwrap();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let editor = editor(&tui, live, app_bindings());
    let view: &dyn maestro_tui::EditorComponent<Signal = maestro_tui::Cancellation> = &editor;
    assert_eq!(
        view.set_autocomplete_provider(Rc::new(Completion)),
        Some(())
    );
    view.input_handler().unwrap().handle_input("\t");
    assert_eq!(runtime.futures(), 1);
    assert!(runtime.drain_futures().is_empty());
    assert_eq!(runtime.futures(), 0);
    assert_eq!(editor.is_showing_autocomplete(), expected.before);
    assert!(editor.render(80).iter().any(|row| row.contains("one")));
    let events = Rc::new(RefCell::new(Vec::new()));
    editor.set_on_escape(Some(callback(&events, "escape")));
    editor.on_action("app.interrupt".into(), callback(&events, "registered"));
    view.input_handler().unwrap().handle_input("\x1b");
    assert_eq!(editor.is_showing_autocomplete(), expected.after);
    view.input_handler().unwrap().handle_input("\x1b");
    assert_eq!(*events.borrow(), expected.events);
    assert_eq!(view.get_text(), expected.text);
    runtime.settle().unwrap();
}
/// Typed query data for custom editor inherits editor and focus capabilities.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CustomEditorInheritsEditorAndFocusCapabilitiesExpected {
    /// Recorded disabled text.
    disabled_text: String,
    /// Recorded text.
    text: String,
    /// Recorded expanded.
    expanded: String,
    /// Recorded padding.
    padding: f64,
    /// Recorded maximum.
    maximum: usize,
    /// Recorded render.
    render: Vec<String>,
    /// Recorded changes.
    changes: Vec<String>,
    /// Recorded submits.
    submits: Vec<String>,
    /// Recorded history.
    history: String,
    /// Recorded stored paste.
    stored_paste: String,
    /// Recorded expanded paste.
    expanded_paste: String,
    /// Recorded updated padding.
    updated_padding: f64,
    /// Recorded updated maximum.
    updated_maximum: usize,
    /// Recorded border sample.
    border_sample: String,
}

#[test]
fn custom_editor_inherits_editor_and_focus_capabilities() {
    let _guard = input_support::globals();
    let [(_, expected)] =
        cases::<EmptyInput, CustomEditorInheritsEditorAndFocusCapabilitiesExpected>(
            "custom_editor_inherits_editor_and_focus_capabilities",
        )
        .try_into()
        .ok()
        .unwrap();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let theme = colors(31, 32, 33, 34);
    let (_state, live) = published::published(&theme);
    let editor = editor(&tui, live, app_bindings());
    let view: &dyn maestro_tui::EditorComponent<Signal = maestro_tui::Cancellation> = &editor;
    let (changes, submits) = inherited_callbacks(view);
    view.set_text("abc");
    view.input_handler().unwrap().handle_input("\x01");
    assert_eq!(view.insert_text_at_cursor("!"), Some(()));
    view.focusable().unwrap().focus_flag().set(true);
    assert!(maestro_tui::Focusable::focus_flag(&editor).get());
    let border: maestro_tui::editor_component::BorderColor =
        Rc::new(move |text| theme.fg(&ThemeColor::Border, text).unwrap());
    assert_eq!(view.set_border_color(border.clone()), Some(()));
    assert!(Rc::ptr_eq(&view.border_color().unwrap(), &border));
    editor.set_disable_submit(true);
    assert!(editor.disable_submit());
    view.input_handler().unwrap().handle_input("\r");
    assert_eq!(view.get_text(), expected.disabled_text);
    editor.set_disable_submit(false);
    view.input_handler().unwrap().handle_input("\r");
    assert_eq!(view.get_text(), expected.text);
    assert_eq!(view.get_expanded_text(), Some(expected.expanded.clone()));
    assert_eq!(editor.get_padding_x().to_bits(), expected.padding.to_bits());
    assert_eq!(editor.get_autocomplete_max_visible(), expected.maximum);
    assert_eq!(view.render(20), expected.render);
    view.invalidate();
    inherited_history_and_paste(&editor, &expected);
    assert_eq!(*changes.borrow(), expected.changes);
    assert_eq!(*submits.borrow(), expected.submits);
    runtime.settle().unwrap();
}

/// Calls a controlled reentrant observer when the last capture is released.
struct OnDrop(Box<dyn Fn()>);
impl Drop for OnDrop {
    fn drop(&mut self) {
        (self.0)();
    }
}
#[test]
fn custom_editor_handlers_release_replaced_values_outside_borrows() {
    let _guard = input_support::globals();
    let runtime = ManualRuntime::new();
    let tui = writer(&runtime);
    let (_state, live) = published::published(&colors(31, 32, 33, 34));
    let editor = Rc::new(editor(&tui, live, app_bindings()));
    let drops = Rc::new(Cell::new(0));
    for kind in 0..4 {
        released_property(&editor, &drops, kind);
    }
    released_map(&editor, &drops);
    direct_property_release(&editor);
}

/// Checks retained property aliases independently from the replaceable callback slot.
fn released_property(
    editor: &Rc<maestro_chat::CustomEditor>,
    drops: &Rc<Cell<usize>>,
    kind: usize,
) {
    let weak = Rc::downgrade(editor);
    let seen = drops.clone();
    let capture = OnDrop(Box::new(move || {
        let editor = weak.upgrade().unwrap();
        assert!(editor.on_escape().is_none());
        assert!(editor.on_ctrl_d().is_none());
        assert!(editor.on_paste_image().is_none());
        assert!(editor.on_extension_shortcut().is_none());
        editor.set_on_escape(None);
        editor.set_on_ctrl_d(None);
        editor.set_on_paste_image(None);
        editor.set_on_extension_shortcut(None);
        seen.set(seen.get() + 1);
    }));
    if kind == 3 {
        let handler: Rc<dyn Fn(&str) -> bool> = Rc::new(move |_| {
            let _retained = &capture;
            false
        });
        editor.set_on_extension_shortcut(Some(handler.clone()));
        let retained = editor.on_extension_shortcut().unwrap();
        assert!(Rc::ptr_eq(&retained, &handler));
        drop(handler);
        editor.set_on_extension_shortcut(None);
        assert_eq!(drops.get(), kind);
        drop(retained);
    } else {
        let handler: Rc<dyn Fn()> = Rc::new(move || {
            let _retained = &capture;
        });
        let retained = match kind {
            0 => {
                editor.set_on_escape(Some(handler.clone()));
                editor.on_escape().unwrap()
            }
            1 => {
                editor.set_on_ctrl_d(Some(handler.clone()));
                editor.on_ctrl_d().unwrap()
            }
            _ => {
                editor.set_on_paste_image(Some(handler.clone()));
                editor.on_paste_image().unwrap()
            }
        };
        assert!(Rc::ptr_eq(&retained, &handler));
        drop(handler);
        match kind {
            0 => editor.set_on_escape(None),
            1 => editor.set_on_ctrl_d(None),
            _ => editor.set_on_paste_image(None),
        }
        assert_eq!(drops.get(), kind);
        drop(retained);
    }
    assert_eq!(drops.get(), kind + 1);
}

/// Checks map-slot replacement and final-alias destruction with reentrant captures.
fn released_map(editor: &Rc<maestro_chat::CustomEditor>, drops: &Rc<Cell<usize>>) {
    let alias = editor.action_handlers();
    let weak = Rc::downgrade(editor);
    let observed = drops.clone();
    let capture = OnDrop(Box::new(move || {
        let editor = weak.upgrade().unwrap();
        editor.on_action("after".into(), Rc::new(|| {}));
        observed.set(observed.get() + 1);
    }));
    editor.on_action(
        "replace".into(),
        Rc::new(move || {
            let _retained = &capture;
        }),
    );
    editor.on_action("replace".into(), Rc::new(|| {}));
    assert_eq!(drops.get(), 5);
    let token = Rc::new(());
    let gone = Rc::downgrade(&token);
    editor.on_action(
        "held".into(),
        Rc::new(move || {
            std::hint::black_box(&token);
        }),
    );
    let retained = alias.clone();
    editor.set_action_handlers(Rc::default());
    assert!(gone.upgrade().is_some());
    drop(alias);
    assert!(gone.upgrade().is_some());
    drop(retained);
    assert!(gone.upgrade().is_none());
    let weak = Rc::downgrade(editor);
    let observed = drops.clone();
    let capture = OnDrop(Box::new(move || {
        let editor = weak.upgrade().unwrap();
        editor.set_action_handlers(Rc::default());
        observed.set(observed.get() + 1);
    }));
    editor.on_action(
        "last".into(),
        Rc::new(move || {
            let _retained = &capture;
        }),
    );
    editor.set_action_handlers(Rc::default());
    assert_eq!(drops.get(), 6);
}

/// Project the disposal recording into its scalar notification format.
fn disposal_events(spy: &Spy) -> Vec<Value> {
    spy.events
        .borrow()
        .iter()
        .map(|event| {
            let row = event.as_array().unwrap();
            if row[0] == "tick" {
                serde_json::json!(row[1].as_f64().unwrap().to_string().parse::<i64>().unwrap())
            } else {
                row[0].clone()
            }
        })
        .collect()
}

/// A distinguishable appended container child.
struct Tail;
impl Component for Tail {
    fn render(&self, _width: usize) -> Vec<String> {
        vec!["tail".into()]
    }
}

/// Installs the competing actions described by a recorded routing query.
fn configure_routing(
    editor: &maestro_chat::CustomEditor,
    manager: &maestro_tui::KeybindingsManager,
    input: &CustomEditorRoutesActionsInSourceOrderInput,
    events: &Rc<RefCell<Vec<Value>>>,
) {
    let mut overrides = Vec::new();
    for (key, action) in [
        (&input.paste, "app.clipboard.pasteImage"),
        (&input.interrupt, "app.interrupt"),
        (&input.exit, "app.exit"),
        (&input.generic, "app.first"),
        (&input.generic, "app.second"),
    ] {
        if let Some(key) = key {
            overrides.push((
                action.into(),
                Some(maestro_tui::KeybindingKeys::Single(key.clone())),
            ));
        }
    }
    manager.set_user_bindings(overrides);
    if let Some(text) = &input.text {
        editor.set_text(text);
    }
    if input.home.unwrap_or(false) {
        editor.input_handler().unwrap().handle_input("\x01");
    }
    if let Some(claims) = input.extension {
        let events = events.clone();
        editor.set_on_extension_shortcut(Some(Rc::new(move |data| {
            events
                .borrow_mut()
                .push(serde_json::json!(["extension", data]));
            claims
        })));
    }
    if input.dynamic.unwrap_or(false) {
        editor.set_on_paste_image(Some(callback(events, "paste")));
        editor.set_on_escape(Some(callback(events, "escape")));
        editor.set_on_ctrl_d(Some(callback(events, "exit")));
    }
    if input.registered.unwrap_or(false) {
        editor.on_action(
            "app.interrupt".into(),
            callback(events, "registered_interrupt"),
        );
        editor.on_action("app.exit".into(), callback(events, "registered_exit"));
    }
    if input.generic.is_some() {
        editor.on_action("app.first".into(), callback(events, "first"));
        editor.on_action("app.second".into(), callback(events, "second"));
    }
}

/// Enumerates the original map's ordered keys.
fn map_keys(map: &maestro_chat::ActionHandlers) -> Vec<String> {
    map.borrow().keys().cloned().collect()
}

/// Exercises inherited history, paste expansion and runtime option setters after submission.
fn inherited_history_and_paste(
    editor: &maestro_chat::CustomEditor,
    expected: &CustomEditorInheritsEditorAndFocusCapabilitiesExpected,
) {
    let view: &dyn maestro_tui::EditorComponent<Signal = maestro_tui::Cancellation> = editor;
    assert_eq!(view.add_to_history("saved"), Some(()));
    view.input_handler().unwrap().handle_input("\x1b[A");
    assert_eq!(view.get_text(), expected.history);
    view.set_text("");
    let paste = format!("{}q", "p\n".repeat(10));
    view.input_handler()
        .unwrap()
        .handle_input(&format!("\x1b[200~{paste}\x1b[201~"));
    assert_eq!(view.get_text(), expected.stored_paste);
    assert_eq!(
        view.get_expanded_text(),
        Some(expected.expanded_paste.clone())
    );
    assert_eq!(view.set_padding_x(3.0), Some(()));
    assert_eq!(view.set_autocomplete_max_visible(4.0), Some(()));
    assert_eq!(
        editor.get_padding_x().to_bits(),
        expected.updated_padding.to_bits()
    );
    assert_eq!(
        editor.get_autocomplete_max_visible(),
        expected.updated_maximum
    );
    assert_eq!(
        view.border_color().unwrap()("sample"),
        expected.border_sample
    );
}

/// A query with no input members.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

/// Reenters each dynamic slot from the destructor that its setter actually releases.
fn direct_property_release(editor: &Rc<maestro_chat::CustomEditor>) {
    let dropped = Rc::new(Cell::new(0));
    for kind in 0..4 {
        let weak = Rc::downgrade(editor);
        let observed = dropped.clone();
        let capture = OnDrop(Box::new(move || {
            let editor = weak.upgrade().unwrap();
            editor.set_on_escape(None);
            editor.set_on_ctrl_d(None);
            editor.set_on_paste_image(None);
            editor.set_on_extension_shortcut(None);
            observed.set(observed.get() + 1);
        }));
        let handler: Rc<dyn Fn()> = Rc::new(move || {
            std::hint::black_box(&capture);
        });
        match kind {
            0 => {
                editor.set_on_escape(Some(handler));
                editor.set_on_escape(None);
            }
            1 => {
                editor.set_on_ctrl_d(Some(handler));
                editor.set_on_ctrl_d(None);
            }
            2 => {
                editor.set_on_paste_image(Some(handler));
                editor.set_on_paste_image(None);
            }
            _ => {
                editor.set_on_extension_shortcut(Some(Rc::new(move |_| {
                    handler();
                    false
                })));
                editor.set_on_extension_shortcut(None);
            }
        }
        assert_eq!(dropped.get(), kind + 1);
    }
}

/// Installs and checks the inherited callback identities before editing starts.
fn inherited_callbacks(
    view: &dyn maestro_tui::EditorComponent<Signal = maestro_tui::Cancellation>,
) -> (StringLog, StringLog) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let change_log = changes.clone();
    let change: maestro_tui::editor_component::TextCallback =
        Rc::new(move |text| change_log.borrow_mut().push(text.to_owned()));
    let submits = Rc::new(RefCell::new(Vec::new()));
    let submit_log = submits.clone();
    let submit: maestro_tui::editor_component::TextCallback =
        Rc::new(move |text| submit_log.borrow_mut().push(text.to_owned()));
    view.set_on_change(Some(change.clone()));
    view.set_on_submit(Some(submit.clone()));
    assert!(Rc::ptr_eq(&view.on_change().unwrap(), &change));
    assert!(Rc::ptr_eq(&view.on_submit().unwrap(), &submit));
    (changes, submits)
}

/// Shared ordered text callback transcript.
type StringLog = Rc<RefCell<Vec<String>>>;

/// Distinguishes insertion precedence from a lexical key sort after map replacement.
fn reverse_lexical_handler_order(
    editor: &maestro_chat::CustomEditor,
    events: &Rc<RefCell<Vec<Value>>>,
) {
    let map = Rc::new(RefCell::new(indexmap::IndexMap::new()));
    map.borrow_mut()
        .insert("app.second".into(), callback(events, "inserted_first"));
    map.borrow_mut()
        .insert("app.first".into(), callback(events, "lexically_first"));
    editor.set_action_handlers(map);
    let start = events.borrow().len();
    editor.input_handler().unwrap().handle_input("x");
    assert_eq!(
        &events.borrow()[start..],
        &[serde_json::json!("inserted_first")]
    );
}

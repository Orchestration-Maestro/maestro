//! Horizontal single-line viewports through the component interface.
use maestro_tui::{Component, Focusable, Input, tui::InputHandler};
#[path = "fixtures/input_support/mod.rs"]
mod input_support;
#[test]
fn wide_scripts_fit_at_start_middle_and_end() {
    let _guard = input_support::globals();
    let positions: [&[&str]; 3] = [&[], &["\x1b[C"; 10], &["\x05"]];
    for (text, expected) in WIDE_CASES {
        for (keys, expected) in positions.iter().zip(expected) {
            let input = Input::new();
            input.set_value((*text).into());
            input.focus_flag().set(true);
            for key in *keys {
                input.handle_input(key);
            }
            assert_eq!(input.render(93), [*expected]);
        }
    }
}
#[test]
fn wide_scroll_retains_the_cursor() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("가나다라마바사아자차카타파하".into());
    input0.focus_flag().set(true);
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    assert_eq!(
        input0.render(20),
        ["> 나다라마\u{1b}_maestro:c\u{7}\u{1b}[7m바\u{1b}[27m사아자  "]
    );
}

#[test]
fn narrow_widths_clip_only_the_prompt() {
    input_support::run("narrow_widths_clip_only_the_prompt");
}

#[test]
fn viewport_boundaries_preserve_exact_cursor_bytes() {
    input_support::run("viewport_boundaries_preserve_exact_cursor_bytes");
}

#[test]
fn cursor_rendering_never_splits_recognized_escapes() {
    input_support::run("cursor_rendering_never_splits_recognized_escapes");
}

#[test]
fn focus_and_invalidation_leave_editing_state_intact() {
    input_support::run("focus_and_invalidation_leave_editing_state_intact");
}

#[test]
fn input_exposes_focus_and_raw_input_capabilities() {
    let _guard = input_support::globals();
    let input: maestro_tui::components::Input = Input::default();
    let component: &dyn Component = &input;
    assert_eq!(input.get_value(), "");
    assert!(!component.focusable().unwrap().focus_flag().get());
    assert!(!component.wants_key_release());
    assert!(input.on_submit().is_none());
    assert!(input.on_escape().is_none());
    component.input_handler().unwrap().handle_input("hello");
    component.focusable().unwrap().focus_flag().set(true);
    assert_eq!(
        component.render(8),
        ["> hello\x1b_maestro:c\x07\x1b[7m \x1b[27m"]
    );
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let events = calls.clone();
    input.set_on_escape(Some(std::rc::Rc::new(move || events.set(events.get() + 1))));
    let retained = input.on_escape().unwrap();
    input.set_on_escape(None);
    retained();
    assert_eq!(calls.get(), 1);
}

/// Wide-script source observations at start, ten graphemes right and end.
const WIDE_CASES: &[(&str, [&str; 3])] = &[
    (
        "가나다라마바사아자차카타파하 한글 텍스트가 터미널 너비를 초과하면 크래시가 발생합니다 이것은 재현용 테스트입니다",
        [
            "> \u{1b}_maestro:c\u{7}\u{1b}[7m가\u{1b}[27m나다라마바사아자차카타파하 한글 텍스트가 터미널 너비를 초과하면 크래시가 발생합니다 이것 ",
            "> 가나다라마바사아자차\u{1b}_maestro:c\u{7}\u{1b}[7m카\u{1b}[27m타파하 한글 텍스트가 터미널 너비를 초과하면 크래시가 발생합니다 이것 ",
            "> 타파하 한글 텍스트가 터미널 너비를 초과하면 크래시가 발생합니다 이것은 재현용 테스트입니다\u{1b}_maestro:c\u{7}\u{1b}[7m \u{1b}[27m",
        ],
    ),
    (
        "これはテスト文章です。日本語のテキストが正しく表示されるかどうかを確認するためのサンプルテキストです。あいうえお",
        [
            "> \u{1b}_maestro:c\u{7}\u{1b}[7mこ\u{1b}[27mれはテスト文章です。日本語のテキストが正しく表示されるかどうかを確認するためのサンプルテ ",
            "> これはテスト文章です\u{1b}_maestro:c\u{7}\u{1b}[7m。\u{1b}[27m日本語のテキストが正しく表示されるかどうかを確認するためのサンプルテ ",
            "> 日本語のテキストが正しく表示されるかどうかを確認するためのサンプルテキストです。あいうえお\u{1b}_maestro:c\u{7}\u{1b}[7m \u{1b}[27m",
        ],
    ),
    (
        "这是一段测试文本，用于验证中文字符在终端中的显示宽度是否被正确计算，如果不正确就会导致用户界面崩溃的问题",
        [
            "> \u{1b}_maestro:c\u{7}\u{1b}[7m这\u{1b}[27m是一段测试文本，用于验证中文字符在终端中的显示宽度是否被正确计算，如果不正确就会导致用户 ",
            "> 这是一段测试文本，用\u{1b}_maestro:c\u{7}\u{1b}[7m于\u{1b}[27m验证中文字符在终端中的显示宽度是否被正确计算，如果不正确就会导致用户 ",
            "> 本，用于验证中文字符在终端中的显示宽度是否被正确计算，如果不正确就会导致用户界面崩溃的问题\u{1b}_maestro:c\u{7}\u{1b}[7m \u{1b}[27m",
        ],
    ),
    (
        "ＡＢＣＤＥＦＧＨＩＪＫＬＭＮＯＰＱＲＳＴＵＶＷＸＹＺ０１２３４５６７８９ａｂｃｄｅｆｇｈｉｊｋｌｍ",
        [
            "> \u{1b}_maestro:c\u{7}\u{1b}[7mＡ\u{1b}[27mＢＣＤＥＦＧＨＩＪＫＬＭＮＯＰＱＲＳＴＵＶＷＸＹＺ０１２３４５６７８９ａｂｃｄｅｆｇｈｉ ",
            "> ＡＢＣＤＥＦＧＨＩＪ\u{1b}_maestro:c\u{7}\u{1b}[7mＫ\u{1b}[27mＬＭＮＯＰＱＲＳＴＵＶＷＸＹＺ０１２３４５６７８９ａｂｃｄｅｆｇｈｉ ",
            "> ＥＦＧＨＩＪＫＬＭＮＯＰＱＲＳＴＵＶＷＸＹＺ０１２３４５６７８９ａｂｃｄｅｆｇｈｉｊｋｌｍ\u{1b}_maestro:c\u{7}\u{1b}[7m \u{1b}[27m",
        ],
    ),
];

#[test]
fn tab_cursor_fits_without_changing_stored_text() {
    let input = Input::new();
    input.set_value("\t".into());
    assert_eq!(input.render(3), ["> \x1b[7m \x1b[27m"]);
    assert_eq!(input.get_value(), "\t");
}

#[test]
fn combining_suffix_reserves_the_mapped_end_cursor() {
    let input = Input::new();
    input.set_value("\u{301}".into());
    input.handle_input("ab");
    assert_eq!(input.render(4), ["> b\u{301}\x1b[7m \x1b[27m"]);
    assert_eq!(input.get_value(), "ab\u{301}");
    input.handle_input("c");
    assert_eq!(input.get_value(), "abc\u{301}");
}

#[test]
fn unterminated_escape_prefixes_remain_literal_at_the_end_cursor() {
    for prefix in ["\x1b[", "\x1b]", "\x1b_"] {
        let input = Input::new();
        let text = prefix.repeat(16_384);
        input.set_value(text.clone());
        input.handle_input("\x05");
        assert_eq!(input.render(3), ["> \x1b[7m \x1b[27m"]);
        assert_eq!(input.get_value(), text);
    }
}

#[test]
fn tabs_display_as_three_spaces_with_the_original_edit_cursor() {
    let input = Input::new();
    input.set_value("a\tb".into());
    input.handle_input("\x1b[C");
    input.handle_input("\x1b[C");
    assert_eq!(input.render(8), ["> a   \x1b[7mb\x1b[27m "]);
    input.handle_input("!");
    assert_eq!(input.get_value(), "a\t!b");
}

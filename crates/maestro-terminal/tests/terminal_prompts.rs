//! Semantic shell phases applied to retained rows and cells.

use super::support::terminal::{assert_text, cell, clear_dirty, dirty, row};
use maestro_terminal::{
    Terminal,
    osc::semantic_prompt::{Action, Command, Redraw},
    page::{cell::SemanticContent, row::SemanticPrompt},
    terminal::SwitchScreenMode,
};

fn phase(t: &mut Terminal, action: Action) {
    t.semantic_prompt(&Command::init(action)).unwrap();
}
fn prompt_row(t: &Terminal, y: u32, expected: SemanticPrompt) {
    assert_eq!(row(t, y).semantic_prompt(), expected);
}
fn newline(t: &mut Terminal) {
    t.carriage_return();
    t.linefeed().unwrap();
}

#[test]
fn editing_semantic_prompt() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::FreshLineNewPrompt);
    t.print_string("hello").unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (5, 0)
    );
    assert_eq!(cell(&t, 4, 0).semantic_content(), SemanticContent::Prompt);
    prompt_row(&t, 0, SemanticPrompt::Prompt);
    phase(&mut t, Action::EndPromptStartInputTerminateEol);
    newline(&mut t);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 1)
    );
    t.print_string("world").unwrap();
    assert_eq!(cell(&t, 4, 1).semantic_content(), SemanticContent::Output);
    prompt_row(&t, 1, SemanticPrompt::None);
}
#[test]
fn editing_semantic_prompt_continuations() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::FreshLineNewPrompt);
    t.print_string("hello").unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (5, 0)
    );
    assert_eq!(cell(&t, 4, 0).semantic_content(), SemanticContent::Prompt);
    prompt_row(&t, 0, SemanticPrompt::Prompt);
    newline(&mut t);
    t.semantic_prompt(&Command {
        action: Action::PromptStart,
        options_unvalidated: b"k=c".to_vec(),
    })
    .unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 1)
    );
    t.print_string("world").unwrap();
    assert_eq!(cell(&t, 4, 1).semantic_content(), SemanticContent::Prompt);
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
}
#[test]
fn editing_index_in_prompt_mode_marks_new_row_as_prompt_continuation() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::PromptStart);
    t.print_string("hello").unwrap();
    prompt_row(&t, 0, SemanticPrompt::Prompt);
    newline(&mut t);
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Prompt
    );
}
#[test]
fn editing_index_in_input_mode_does_not_mark_new_row_as_prompt() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::PromptStart);
    t.print_string("$ ").unwrap();
    phase(&mut t, Action::EndPromptStartInput);
    t.print_string("echo \\").unwrap();
    newline(&mut t);
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Input
    );
}
#[test]
fn editing_index_in_output_mode_does_not_mark_new_row_as_prompt() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::PromptStart);
    t.print_string("$ ").unwrap();
    phase(&mut t, Action::EndPromptStartInput);
    t.print_string("ls").unwrap();
    phase(&mut t, Action::EndInputStartOutput);
    newline(&mut t);
    prompt_row(&t, 1, SemanticPrompt::None);
}
#[test]
fn editing_osc133_c_at_x_0_on_prompt_row_clears_prompt_mark() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::PromptStart);
    t.print_string("$ echo \\").unwrap();
    newline(&mut t);
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
    assert_eq!(t.screens.active().cursor.x, 0);
    phase(&mut t, Action::EndInputStartOutput);
    prompt_row(&t, 1, SemanticPrompt::None);
}
#[test]
fn editing_osc133_c_at_x_0_on_prompt_row_does_not_clear_prompt_mark() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::PromptStart);
    t.print_string("$ ").unwrap();
    newline(&mut t);
    t.semantic_prompt(&Command {
        action: Action::PromptStart,
        options_unvalidated: b"k=c".to_vec(),
    })
    .unwrap();
    t.print_string("> ").unwrap();
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
    assert!(t.screens.active().cursor.x > 0);
    phase(&mut t, Action::EndInputStartOutput);
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
}
#[test]
fn editing_multiple_newlines_in_prompt_mode_marks_all_rows() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    phase(&mut t, Action::PromptStart);
    t.print_string("line1").unwrap();
    newline(&mut t);
    t.print_string("line2").unwrap();
    newline(&mut t);
    t.print_string("line3").unwrap();
    prompt_row(&t, 0, SemanticPrompt::Prompt);
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
    prompt_row(&t, 2, SemanticPrompt::PromptContinuation);
}
#[test]
fn editing_cursor_is_at_prompt() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    assert!(!t.cursor_is_at_prompt());
    phase(&mut t, Action::PromptStart);
    assert!(t.cursor_is_at_prompt());
    t.print_string("$ ").unwrap();
    phase(&mut t, Action::EndPromptStartInput);
    assert!(t.cursor_is_at_prompt());
    t.print_string("ls").unwrap();
    phase(&mut t, Action::EndInputStartOutput);
    assert!(t.cursor_is_at_prompt());
    t.linefeed().unwrap();
    assert!(!t.cursor_is_at_prompt());
    t.linefeed().unwrap();
    phase(&mut t, Action::PromptStart);
    assert!(t.cursor_is_at_prompt());
}
#[test]
fn editing_cursor_is_at_prompt_alternate_screen() {
    let mut t = Terminal::new(3, 2, 0).unwrap();
    assert!(!t.cursor_is_at_prompt());
    phase(&mut t, Action::PromptStart);
    assert!(t.cursor_is_at_prompt());
    t.switch_screen_mode(SwitchScreenMode::Mode1049, true)
        .unwrap();
    assert!(!t.cursor_is_at_prompt());
    phase(&mut t, Action::PromptStart);
    assert!(!t.cursor_is_at_prompt());
}
#[test]
fn editing_soft_wrap_with_semantic_prompt() {
    let mut t = Terminal::new(3, 80, 0).unwrap();
    clear_dirty(&mut t);
    phase(&mut t, Action::PromptStart);
    assert!(!dirty(&t, 0));
    t.print_string("hello").unwrap();
    prompt_row(&t, 0, SemanticPrompt::Prompt);
    prompt_row(&t, 1, SemanticPrompt::PromptContinuation);
}
#[test]
fn fresh_prompt_line_respects_horizontal_margins() {
    use maestro_terminal::Mode;
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(4, 8);
    for (x, expected_x, expected_y) in [(0, 0, 0), (1, 0, 1), (3, 3, 0), (5, 3, 1), (9, 3, 1)] {
        t.set_cursor_pos(1, x + 1);
        phase(&mut t, Action::FreshLine);
        assert_eq!(
            (
                usize::from(t.screens.active().cursor.x),
                t.screens.active().cursor.y
            ),
            (expected_x, expected_y)
        );
    }
    t.set_cursor_pos(5, 8);
    t.screens.active_mut().cursor.pending_wrap = true;
    phase(&mut t, Action::PromptStart);
    phase(&mut t, Action::FreshLine);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (3, 4)
    );
    assert!(!t.screens.active().cursor.pending_wrap);
    prompt_row(&t, 4, SemanticPrompt::PromptContinuation);
}
#[test]
fn semantic_actions_cover_all_prompt_phases() {
    use maestro_terminal::{
        osc::semantic_prompt::PromptKind,
        screen::{InputTermination, SemanticContent},
    };
    let mut t = Terminal::new(10, 5, 0).unwrap();
    for (raw, expected) in [
        (b"k=i".as_slice(), SemanticPrompt::Prompt),
        (b"k=r", SemanticPrompt::Prompt),
        (b"k=c", SemanticPrompt::PromptContinuation),
        (b"k=s", SemanticPrompt::PromptContinuation),
    ] {
        t.semantic_prompt(&Command {
            action: Action::PromptStart,
            options_unvalidated: raw.to_vec(),
        })
        .unwrap();
        prompt_row(&t, 0, expected);
        assert!(t.screens.active().semantic_prompt.seen);
    }
    t.screens
        .active_mut()
        .cursor_set_semantic_content(SemanticContent::Prompt(PromptKind::Initial));
    for (action, clear) in [
        (Action::EndPromptStartInput, false),
        (Action::EndPromptStartInputTerminateEol, true),
    ] {
        phase(&mut t, action);
        assert_eq!(
            t.screens.active().cursor.semantic_content,
            maestro_terminal::page::cell::SemanticContent::Input
        );
        assert_eq!(t.screens.active().cursor.semantic_content_clear_eol, clear);
    }
    t.screens
        .active_mut()
        .cursor_set_semantic_content(SemanticContent::Input(InputTermination::ClearExplicit));
    phase(&mut t, Action::EndCommand);
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        maestro_terminal::page::cell::SemanticContent::Output
    );
    assert!(!t.screens.active().cursor.semantic_content_clear_eol);
    t.print('x').unwrap();
    phase(&mut t, Action::NewCommand);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 1)
    );
    prompt_row(&t, 1, SemanticPrompt::Prompt);
    prompt_option_controls(&mut t);
}

fn prompt_option_controls(t: &mut Terminal) {
    for raw in [b"k=bad;k=c".as_slice(), b"k=i;k=c", b"unknown=x"] {
        t.semantic_prompt(&Command {
            action: Action::PromptStart,
            options_unvalidated: raw.to_vec(),
        })
        .unwrap();
        prompt_row(t, 1, SemanticPrompt::Prompt);
    }
    t.semantic_prompt(&Command {
        action: Action::NewCommand,
        options_unvalidated: b"k=s;redraw=last".to_vec(),
    })
    .unwrap();
    prompt_row(t, 1, SemanticPrompt::PromptContinuation);
    assert_eq!(t.flags.shell_redraws_prompt, Redraw::Last);
}
#[test]
fn explicit_redraw_policies_reach_primary_resize() {
    use maestro_terminal::screen_set::Key;
    for redraw in [Redraw::False, Redraw::True, Redraw::Last] {
        let mut t = Terminal::new(10, 5, 0).unwrap();
        t.full_reset();
        let raw = match redraw {
            Redraw::False => b"redraw=0".as_slice(),
            Redraw::True => b"redraw=1",
            Redraw::Last => b"redraw=last",
        };
        t.semantic_prompt(&Command {
            action: Action::FreshLineNewPrompt,
            options_unvalidated: raw.to_vec(),
        })
        .unwrap();
        t.print_string("first").unwrap();
        newline(&mut t);
        t.print_string("$ ").unwrap();
        assert_eq!(t.flags.shell_redraws_prompt, redraw);
        t.switch_screen(Key::Alternate);
        t.set_cursor_pos(1, 1);
        phase(&mut t, Action::PromptStart);
        t.print_string("alt").unwrap();
        t.resize(12, 5, 1, 1).unwrap();
        assert_text(&t, "alt");
        t.switch_screen(Key::Primary);
        assert_text(
            &t,
            match redraw {
                Redraw::False => "first\n$ ",
                Redraw::True => "",
                Redraw::Last => "first",
            },
        );
    }
}

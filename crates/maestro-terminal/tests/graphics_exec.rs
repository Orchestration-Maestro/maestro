//! Image execution through terminal commands and the byte-consumer seam.

use maestro_terminal::{
    Terminal,
    kitty::graphics::{
        command::{Parser, Response},
        image::Limits,
    },
};

/// Execute the parsed controls without bypassing terminal-owned storage.
fn execute(terminal: &mut Terminal, bytes: &[u8]) -> Option<Response> {
    terminal.kitty_graphics(&Parser::parse_string(bytes).unwrap())
}

/// A configured terminal with known cell pixels.
fn terminal() -> Terminal {
    let mut terminal = Terminal::new(5, 5, 100_000).unwrap();
    terminal.resize(5, 5, 10, 20).unwrap();
    terminal.enable_kitty_graphics();
    terminal
}

#[test]
fn chunked_upload_inherits_and_increases_quiet_policy() {
    for (initial, final_chunk, replies) in [
        (
            b"a=T,f=24,i=1,s=1,v=2,c=2,r=1,m=1,q=1;////".as_slice(),
            b"m=0;////".as_slice(),
            false,
        ),
        (b"a=t,f=24,i=1,s=1,v=2,m=1,q=0;////", b"m=0;////", true),
        (b"a=t,f=24,i=1,s=1,v=2,m=1,q=0;////", b"m=0,q=1;////", false),
    ] {
        let mut terminal = terminal();
        assert!(execute(&mut terminal, initial).is_none());
        let response = execute(&mut terminal, final_chunk);
        assert_eq!(response.is_some(), replies);
        assert!(response.is_none_or(|r| r.ok()));
        assert_eq!(
            terminal.screens.active().kitty_images.images[&1].data,
            [255; 6]
        );
        assert!(terminal.screens.active().kitty_images.loading.is_none());
    }
}

#[test]
fn default_format_is_rgba_and_identifiers_retain_full_width() {
    let mut terminal = terminal();
    assert!(
        execute(&mut terminal, b"a=t,i=1,s=1,v=2;//////////8=")
            .unwrap()
            .ok()
    );
    assert_eq!(
        terminal.screens.active().kitty_images.images[&1].format,
        maestro_terminal::kitty::graphics::command::Format::Rgba
    );
    for command in [b"a=p,i=4294967295".as_slice(), b"a=p,i=2,z=-2147483648"] {
        assert_eq!(
            execute(&mut terminal, command).unwrap().message,
            b"ENOENT: image not found"
        );
    }
}

#[test]
fn anonymous_uploads_do_not_reply_even_when_displayed() {
    for action in [
        b"a=t,f=24,s=1,v=2;////////".as_slice(),
        b"a=T,f=24,s=1,v=2,c=1,r=1;////////",
    ] {
        let mut terminal = terminal();
        assert!(execute(&mut terminal, action).is_none());
        let storage = &terminal.screens.active().kitty_images;
        assert_eq!(storage.images.len(), 1);
        assert!(storage.images.values().next().unwrap().implicit_id);
    }
}

#[test]
fn retransmission_and_deletion_never_reuse_generation() {
    let mut terminal = terminal();
    let upload = b"a=T,f=24,i=1,s=1,v=2,c=1,r=1;////////";
    execute(&mut terminal, upload).unwrap();
    let first = terminal.screens.active().kitty_images.images[&1].generation;
    execute(&mut terminal, b"a=t,f=24,i=1,s=1,v=2;AAAAAAAA").unwrap();
    let second = terminal.screens.active().kitty_images.images[&1].generation;
    assert!(second > first);
    assert_eq!(
        terminal.screens.active().kitty_images.images[&1].data,
        [0; 6]
    );
    assert!(execute(&mut terminal, b"a=d,d=A").is_none());
    let deleted = terminal.screens.active().kitty_images.generation;
    assert!(deleted > second);
    assert!(terminal.screens.active().kitty_images.images.is_empty());
    execute(&mut terminal, upload).unwrap();
    assert!(terminal.screens.active().kitty_images.images[&1].generation > deleted);
}

#[test]
fn apc_responses_and_display_use_the_existing_terminal_write_route() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Ga=q,i=31,s=1,v=1,f=24;AAAA\x1b\\");
    assert_eq!(terminal.take_pty_reply(), b"\x1b_Gi=31;OK\x1b\\");
    assert!(terminal.screens.active().kitty_images.images.is_empty());
    terminal.write(b"\x1b_Ga=T,i=31,s=1,v=1,f=24,c=2,r=1;AAAA\x1b\\");
    assert_eq!(terminal.take_pty_reply(), b"\x1b_Gi=31;OK\x1b\\");
    let screen = terminal.screens.active();
    assert_eq!(screen.kitty_images.images[&31].data, [0; 3]);
    assert_eq!(screen.kitty_images.placements.len(), 1);
    assert_eq!((screen.cursor.x, screen.cursor.y), (2, 1));
}

#[test]
fn display_resolves_numbers_and_preserves_cursor_for_virtual_or_stationary_placements() {
    let mut terminal = terminal();
    let upload = execute(&mut terminal, b"a=t,I=9,f=24,s=1,v=1;AAAA").unwrap();
    assert_ne!(upload.id, 0);
    assert_eq!(upload.image_number, 9);
    let displayed = execute(&mut terminal, b"a=p,I=9,p=2,c=2,r=2,C=1").unwrap();
    assert_eq!(displayed.id, upload.id);
    assert!(displayed.ok());
    assert!(
        execute(&mut terminal, b"a=p,I=9,U=1,P=1")
            .unwrap()
            .message
            .starts_with(b"EINVAL:")
    );
    assert!(
        execute(&mut terminal, b"a=p,I=9,U=1,p=3,c=1,r=1")
            .unwrap()
            .ok()
    );
    let screen = terminal.screens.active();
    assert_eq!((screen.cursor.x, screen.cursor.y), (0, 0));
    assert_eq!(screen.kitty_images.placements.len(), 2);
}

#[test]
fn display_indexes_within_scroll_region() {
    let mut terminal = terminal();
    terminal.set_top_and_bottom_margin(2, 4);
    terminal.set_cursor_pos(4, 2);
    execute(&mut terminal, b"a=T,i=1,f=24,s=1,v=1,c=2,r=2;AAAA").unwrap();
    assert_eq!(terminal.screens.active().cursor.y, 3);
    assert_eq!(terminal.screens.active().cursor.x, 3);
    assert!(terminal.screens.active().kitty_images.dirty);
}

#[test]
fn quiet_errors_query_and_animation_follow_response_policy() {
    let mut terminal = terminal();
    assert!(execute(&mut terminal, b"a=q,s=1,v=1,f=24;AAAA").is_none());
    assert!(
        execute(&mut terminal, b"a=q,i=1,f=24,s=1,v=1;AAAA")
            .unwrap()
            .ok()
    );
    assert_eq!(
        execute(&mut terminal, b"a=t,i=1,f=24;AAAA")
            .unwrap()
            .message,
        b"EINVAL: dimensions required"
    );
    assert!(execute(&mut terminal, b"a=p,i=3,q=1").is_some());
    assert!(execute(&mut terminal, b"a=p,i=3,q=2").is_none());
    assert!(execute(&mut terminal, b"a=t,i=1,I=2").is_none());
    for command in [b"a=f,i=1,q=1".as_slice(), b"a=a,i=1,q=1", b"a=c,i=1,q=1"] {
        let response = execute(&mut terminal, command).unwrap();
        assert_eq!(response.message, b"ERROR: unimplemented action");
        assert!(response.empty());
    }
}

#[test]
fn png_decoding_follows_each_terminals_setting() {
    use maestro_terminal::kitty::graphics::command::{
        Command, Control, Format, Quiet, Transmission,
    };
    let mut enabled = terminal();
    let mut disabled = Terminal::new(5, 5, 0).unwrap();
    // Admit raw images without enabling this terminal's PNG decoder.
    disabled.screens.active_mut().kitty_images.total_limit = 1024;
    let command = Command {
        control: Control::Transmit(Transmission {
            image_id: 1,
            format: Format::Png,
            ..Transmission::default()
        }),
        quiet: Quiet::No,
        data: include_bytes!("fixtures/images/rgba8.png").to_vec(),
    };
    assert!(enabled.kitty_graphics(&command).unwrap().ok());
    enabled.write(b"\x1bc");
    assert!(enabled.kitty_graphics(&command).unwrap().ok());
    assert_eq!(
        disabled.kitty_graphics(&command).unwrap().message,
        b"EINVAL: unsupported format"
    );
    assert!(disabled.screens.active().kitty_images.images.is_empty());
    disabled.enable_kitty_graphics();
    assert!(disabled.kitty_graphics(&command).unwrap().ok());
}

#[test]
fn ris_clears_images_without_changing_host_policy() {
    for enabled in [false, true] {
        let mut terminal = terminal();
        execute(&mut terminal, b"a=T,i=1,f=24,s=1,v=1,c=1,r=1;AAAA").unwrap();
        let screen = terminal.screens.active_mut();
        if !enabled {
            screen.kitty_images.set_limit(&mut screen.pages, 0);
        }
        let before = screen.kitty_images.generation;
        let baseline = screen.pages.tracked_pins().count() - screen.kitty_images.placements.len();
        terminal.write(b"\x1bc");
        let screen = terminal.screens.active();
        assert_eq!(
            screen.kitty_images.total_limit,
            if enabled { 64 * 1024 * 1024 } else { 0 }
        );
        assert_eq!(screen.kitty_images.image_limits, Limits::ALL);
        assert!(screen.kitty_images.images.is_empty());
        assert!(screen.kitty_images.placements.is_empty());
        assert!(screen.kitty_images.generation > before);
        assert_eq!(screen.pages.tracked_pins().count(), baseline);
        let response = execute(&mut terminal, b"a=q,i=1");
        assert_eq!(response.is_some(), enabled);
    }
    let mut never_enabled = Terminal::new(5, 5, 0).unwrap();
    never_enabled.write(b"\x1bc\x1b_Ga=q,i=1\x1b\\");
    assert!(never_enabled.take_pty_reply().is_empty());
    assert_eq!(
        never_enabled.screens.active().kitty_images.image_limits,
        Limits::DIRECT
    );
}

#[test]
fn screen_switch_inherits_policy_and_scrolling_and_resize_dirty_images() {
    use maestro_terminal::{page_list::Scroll, screen_set::Key};
    let mut terminal = terminal();
    terminal.switch_screen(Key::Alternate);
    assert_eq!(
        terminal.screens.active().kitty_images.total_limit,
        64 * 1024 * 1024
    );
    assert_eq!(
        terminal.screens.active().kitty_images.image_limits,
        Limits::ALL
    );
    let storage = &mut terminal.screens.active_mut().kitty_images;
    storage.dirty = false;
    terminal.screens.active_mut().scroll(Scroll::Active);
    assert!(terminal.screens.active().kitty_images.dirty);
    terminal.screens.active_mut().kitty_images.dirty = false;
    terminal.resize(6, 5, 10, 20).unwrap();
    assert!(terminal.screens.active().kitty_images.dirty);
}

#[test]
fn complete_erasure_removes_images_but_preserves_virtual_prototypes() {
    use maestro_terminal::csi::EraseDisplay;
    let mut terminal = terminal();
    execute(&mut terminal, b"a=T,i=1,f=24,s=1,v=1,c=1,r=1;AAAA").unwrap();
    execute(&mut terminal, b"a=p,i=1,U=1,p=2,c=1,r=1").unwrap();
    terminal.erase_display(EraseDisplay::Complete, false);
    assert_eq!(terminal.screens.active().kitty_images.placements.len(), 1);
    assert_eq!(terminal.screens.active().kitty_images.images.len(), 1);
}

#[test]
fn display_movement_does_not_reapply_origin_offsets() {
    let mut terminal = terminal();
    terminal.set_top_and_bottom_margin(3, 5);
    terminal.modes.set(maestro_terminal::Mode::Origin, true);
    terminal.set_cursor_pos(1, 1);
    execute(&mut terminal, b"a=T,i=1,f=24,s=1,v=1,c=2,r=1;AAAA").unwrap();
    let screen = terminal.screens.active();
    assert_eq!((screen.cursor.x, screen.cursor.y), (2, 3));
}

#[test]
fn failed_uploads_release_completion_state_and_report_quota_errors() {
    let mut terminal = terminal();
    let screen = terminal.screens.active_mut();
    screen.kitty_images.set_limit(&mut screen.pages, 2);
    assert_eq!(
        execute(&mut terminal, b"a=t,i=1,f=24,s=1,v=1;AAAA")
            .unwrap()
            .message,
        b"ENOMEM: out of memory"
    );
    let screen = terminal.screens.active_mut();
    screen
        .kitty_images
        .set_limit(&mut screen.pages, 64 * 1024 * 1024);
    assert!(execute(&mut terminal, b"a=t,i=1,f=24,s=1,v=1,m=1;AAAA").is_none());
    assert_eq!(
        execute(&mut terminal, b"m=0,i=1;AA==").unwrap().message,
        b"EINVAL: invalid data"
    );
    assert!(terminal.screens.active().kitty_images.loading.is_none());
    assert!(
        execute(&mut terminal, b"a=t,i=1,f=24,s=1,v=1;AAAA")
            .unwrap()
            .ok()
    );
}

#[test]
fn row_shifts_and_partial_height_history_scroll_dirty_images() {
    let mut terminal = terminal();
    terminal.set_top_and_bottom_margin(1, 3);
    terminal.screens.active_mut().kitty_images.dirty = false;
    terminal.scroll_up(1).unwrap();
    assert!(terminal.screens.active().kitty_images.dirty);
    terminal.set_cursor_pos(2, 1);
    terminal.screens.active_mut().kitty_images.dirty = false;
    terminal.insert_lines(1).unwrap();
    assert!(terminal.screens.active().kitty_images.dirty);
    terminal.screens.active_mut().kitty_images.dirty = false;
    terminal.delete_lines(1).unwrap();
    assert!(terminal.screens.active().kitty_images.dirty);
    terminal.set_cursor_pos(3, 1);
    terminal.screens.active_mut().kitty_images.dirty = false;
    terminal.index().unwrap();
    assert!(terminal.screens.active().kitty_images.dirty);
}

#[test]
fn zero_height_display_clears_pending_wrap_only_when_moving() {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode([0; 300]);
    for stationary in [true, false] {
        let mut terminal = terminal();
        terminal.write(b"abcde");
        assert!(terminal.screens.active().cursor.pending_wrap);
        terminal.write(
            format!(
                "\x1b_Ga=T,f=24,i=1,s=100,v=1,c=1,C={},q=2;{data}\x1b\\",
                u8::from(stationary)
            )
            .as_bytes(),
        );
        let pending_wrap = terminal.screens.active().cursor.pending_wrap;
        terminal.write(b"X");
        assert_eq!(
            (
                pending_wrap,
                terminal
                    .screen_cell(if stationary { 0 } else { 4 }, u32::from(stationary))
                    .unwrap()
                    .1
            ),
            (stationary, vec!['X']),
            "display must settle pending wrap before the next byte"
        );
    }
}

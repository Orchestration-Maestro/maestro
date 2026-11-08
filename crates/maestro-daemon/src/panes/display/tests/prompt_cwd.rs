//! Prompt-derived cwd policy with explicitly enabled owned state.
use super::super::{prompt_cwd::windows_powershell_prompt_cwd, *};
use maestro_terminal::Terminal;
use maestro_workspace::tiling::PaneId;
#[test]
fn pane_windows_powershell_prompt_cwd_uses_latest_default_prompt_path() {
    let cwd = std::env::current_dir().unwrap();
    let text = format!("PS C:\\old> cd {}\nPS {}>", cwd.display(), cwd.display());

    assert_eq!(windows_powershell_prompt_cwd(&text).as_ref(), Some(&cwd));
}

#[test]
fn pane_windows_powershell_prompt_cwd_requires_current_prompt_line() {
    let cwd = std::env::current_dir().unwrap();
    let text = format!("PS {}>\ncommand output", cwd.display());

    assert_eq!(windows_powershell_prompt_cwd(&text), None);
}

#[test]
fn pane_windows_powershell_prompt_cwd_ignores_command_echo() {
    let cwd = std::env::current_dir().unwrap();
    let text = format!("PS {}> echo hi", cwd.display());

    assert_eq!(windows_powershell_prompt_cwd(&text), None);
}

fn process_windows_powershell_prompt_bytes(
    bytes: &[u8],
    cols: u16,
    rows: u16,
    enabled: bool,
) -> ProcessBytesResult {
    let terminal = Terminal::new(cols, rows, 100).unwrap();
    let pane = PaneTerminal::new(terminal);
    pane.set_windows_powershell_prompt_cwd_reporting(enabled);
    pane.process_pty_bytes(PaneId::from_raw(1), 0, bytes, || None)
}

#[test]
fn pane_process_pty_bytes_reports_windows_powershell_prompt_cwd() {
    let cwd = std::env::current_dir().unwrap();
    let bytes = format!("PS C:\\old> cd {}\r\nPS {}>", cwd.display(), cwd.display());

    let result = process_windows_powershell_prompt_bytes(bytes.as_bytes(), 80, 24, true);

    assert_eq!(result.reported_cwd.as_ref(), Some(&cwd));
}

#[test]
fn pane_process_pty_bytes_reports_wrapped_windows_powershell_prompt_cwd() {
    let cwd = std::env::current_dir().unwrap();
    let bytes = format!("PS {}>", cwd.display());

    let result = process_windows_powershell_prompt_bytes(bytes.as_bytes(), 12, 8, true);

    assert_eq!(result.reported_cwd.as_ref(), Some(&cwd));
}

#[test]
fn pane_process_pty_bytes_ignores_prompt_like_output_on_previous_line() {
    let cwd = std::env::current_dir().unwrap();
    let bytes = format!("PS {}>\r\n", cwd.display());

    let result = process_windows_powershell_prompt_bytes(bytes.as_bytes(), 80, 24, true);

    assert_eq!(result.reported_cwd, None);
}

#[test]
fn pane_process_pty_bytes_ignores_windows_powershell_prompt_cwd_on_alternate_screen() {
    let cwd = std::env::current_dir().unwrap();
    let bytes = format!("\x1b[?1049hPS {}>", cwd.display());

    let result = process_windows_powershell_prompt_bytes(bytes.as_bytes(), 80, 24, true);

    assert_eq!(result.reported_cwd, None);
}

#[test]
fn pane_process_pty_bytes_skips_windows_powershell_prompt_cwd_when_disabled() {
    let cwd = std::env::current_dir().unwrap();
    let bytes = format!("PS {}>", cwd.display());

    let result = process_windows_powershell_prompt_bytes(bytes.as_bytes(), 80, 24, false);

    assert_eq!(result.reported_cwd, None);
}

/// Cells consumed by a prompt, including wide-character wrap spacers.
fn prompt_cells(path: &std::path::Path) -> usize {
    let text = format!("PS {}>", path.display());
    let rows = u16::try_from(text.len().div_ceil(12) + 1).unwrap();
    let mut terminal = Terminal::new(12, rows, 0).unwrap();
    terminal.print_string(&text).unwrap();
    let cursor = &terminal.screens.active().cursor;
    usize::from(cursor.y) * 12 + usize::from(cursor.x) + usize::from(cursor.pending_wrap)
}

#[test]
fn prompt_cells_includes_wide_character_wrap_spacer() {
    assert_eq!(prompt_cells(std::path::Path::new("/tmp/abc界")), 15);
}

/// Native directories owned by one prompt test, without changing ambient cwd.
struct PromptDirectories(maestro_test_directory::Scratch);

impl PromptDirectories {
    /// Exclusively create a root distinct from parallel prompt tests.
    fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }
    /// The root this test owns.
    fn root(&self) -> &std::path::Path {
        self.0.path()
    }

    /// Keep twelve columns while accommodating unusually long temporary roots.
    fn rows(&self) -> u16 {
        u16::try_from((prompt_cells(self.root()) + 2).div_ceil(12).max(8)).unwrap()
    }

    /// Extend the native root with ASCII components to reach the requested prompt cells.
    fn directory(&self, requested_cells: usize) -> std::path::PathBuf {
        let mut path = self.root().to_path_buf();
        while prompt_cells(&path) < requested_cells {
            let remaining = requested_cells - prompt_cells(&path);
            let component = if remaining > 65 { 60 } else { remaining - 1 };
            path.push("x".repeat(component));
        }
        std::fs::create_dir_all(&path).unwrap();
        assert_eq!(prompt_cells(&path), requested_cells);
        path
    }
}

#[test]
fn pane_process_pty_bytes_recovers_prompt_cwd_across_screen_capacity() {
    let directories = PromptDirectories::new();
    let rows = directories.rows();
    let capacity = usize::from(rows) * 12;
    for cells in [capacity - 1, capacity, capacity + 1, capacity * 2 + 1] {
        let cwd = directories.directory(cells);
        let bytes = format!("PS {}>", cwd.display());
        let pane = PaneTerminal::new(Terminal::new(12, rows, 100).unwrap());
        pane.set_windows_powershell_prompt_cwd_reporting(true);
        let result = pane.process_pty_bytes(PaneId::from_raw(1), 0, bytes.as_bytes(), || None);
        {
            let core = pane.core.lock().unwrap();
            if cells > capacity {
                assert!(core.terminal.total_rows() > usize::from(rows));
            }
            assert_eq!(
                core.terminal.total_rows(),
                cells.div_ceil(12).max(usize::from(rows)),
                "cells={cells}"
            );
            let cursor = &core.terminal.screens.active().cursor;
            assert_eq!(
                usize::from(cursor.x) + usize::from(cursor.pending_wrap),
                (cells - 1) % 12 + 1,
                "cells={cells}"
            );
            assert_eq!(cursor.pending_wrap, cells % 12 == 0, "cells={cells}");
        }
        assert_eq!(result.reported_cwd.as_ref(), Some(&cwd), "cells={cells}");
    }
}

#[test]
fn pane_process_pty_bytes_preserves_prompt_guards_after_scrolling() {
    let directories = PromptDirectories::new();
    let rows = directories.rows();
    let cwd = directories.directory(usize::from(rows) * 24 + 1);
    let prompt = format!("PS {}>", cwd.display());
    for (case, bytes, enabled, history) in [
        ("blank row", format!("{prompt}\r\n"), true, 100),
        ("command echo", format!("{prompt} echo hi"), true, 100),
        ("output", format!("{prompt}\r\ncommand output"), true, 100),
        ("alternate", format!("\x1b[?1049h{prompt}"), true, 100),
        ("disabled", prompt.clone(), false, 100),
        ("pruned prefix", prompt, true, 0),
    ] {
        let pane = PaneTerminal::new(Terminal::new(12, rows, history).unwrap());
        pane.set_windows_powershell_prompt_cwd_reporting(enabled);
        let result = pane.process_pty_bytes(PaneId::from_raw(1), 0, bytes.as_bytes(), || None);
        assert_eq!(result.reported_cwd, None, "{case}");
    }
}

#[test]
fn pane_windows_powershell_prompt_cwd_preserves_directory_grammar() {
    let directories = PromptDirectories::new();
    let cwd = directories.root().to_path_buf();
    let file = cwd.join("ordinary-file");
    std::fs::File::create_new(&file).unwrap();
    for (case, text) in [
        ("empty", String::new()),
        ("blank", "\n \t\n".to_owned()),
        ("missing prefix", format!("{}>", cwd.display())),
        ("missing marker", format!("PS {}", cwd.display())),
        ("missing directory", "PS >".to_owned()),
        ("relative directory", "PS .>".to_owned()),
        (
            "missing absolute directory",
            format!("PS {}>", cwd.join("missing").display()),
        ),
        ("ordinary file", format!("PS {}>", file.display())),
        ("command", format!("PS {}> echo hi", cwd.display())),
        ("invalid suffix", format!("PS {}>!", cwd.display())),
    ] {
        assert_eq!(windows_powershell_prompt_cwd(&text), None, "{case}");
    }
    for (case, text) in [
        ("marker whitespace", format!("PS {} \t> \t", cwd.display())),
        (
            "filesystem provider",
            format!("PS FileSystem::{}>", cwd.display()),
        ),
        ("repeated markers", format!("PS {}>>>", cwd.display())),
        ("blank tail", format!("PS {}>\n \t", cwd.display())),
    ] {
        assert_eq!(
            windows_powershell_prompt_cwd(&text).as_ref(),
            Some(&cwd),
            "{case}"
        );
    }
}

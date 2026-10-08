//! A writer over an emulated terminal, driven the way an application drives it.

use std::rc::Rc;

use maestro_tui::{TUI, TerminalImage};
use serde::Deserialize;

use super::checks::{parse, succeeds};
use super::components::Probe;
use super::manual_runtime::ManualRuntime;
use super::virtual_terminal::VirtualTerminal;

/// A writer over an emulated terminal, driven the way an application drives it.
pub struct Scene {
    /// The emulated terminal.
    pub terminal: VirtualTerminal,
    /// The controlled host.
    pub runtime: ManualRuntime,
    /// The writer under test.
    pub tui: TUI,
    /// The only component.
    pub probe: Rc<Probe>,
}

impl Scene {
    /// A scene over a blank terminal of the given size whose writer is not started.
    pub fn new(columns: usize, rows: usize) -> Self {
        Self::with_images(columns, rows, TerminalImage::new(|_| None, || 1))
    }

    /// A scene whose components share the image state `images`.
    pub fn with_images(columns: usize, rows: usize, images: TerminalImage) -> Self {
        let terminal = VirtualTerminal::new(columns, rows);
        let runtime = ManualRuntime::new();
        let probe = Probe::shared(&[]);
        let tui = TUI::new(terminal.handle(), runtime.handle(), images, None);
        tui.add_child(probe.clone());
        Self {
            terminal,
            runtime,
            tui,
            probe,
        }
    }

    /// Starts the writer and draws `lines`.
    pub fn start(&self, lines: &[&str]) {
        succeeds(self.tui.start());
        self.show(lines);
    }

    /// Replaces the content and draws it.
    pub fn show(&self, lines: &[&str]) {
        let lines: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
        self.probe.set_lines(&lines);
        self.render();
    }

    /// Draws the current content again.
    pub fn render(&self) {
        self.tui.request_render(false);
        succeeds(self.runtime.settle());
    }

    /// Redraws the current content from scratch.
    pub fn redraw(&self) {
        self.tui.request_render(true);
        succeeds(self.runtime.settle());
    }

    /// Resizes the terminal and lets the writer react.
    pub fn resize(&self, columns: usize, rows: usize) {
        self.terminal.resize(columns, rows);
        succeeds(self.runtime.settle());
    }

    /// The visible rows of the emulated screen.
    pub fn viewport(&self) -> Vec<String> {
        self.terminal.viewport()
    }

    /// Stops the writer.
    pub fn stop(&self) {
        succeeds(self.tui.stop());
    }

    /// Asserts that the writes so far are the ones recorded for `scene`, ignoring cursor
    /// visibility escapes.
    pub fn assert_recorded(&self, scene: &str) {
        let scenes: Vec<RecordedScene> = parse(include_str!("../fixtures/scenes.json"));
        let recorded = scenes.into_iter().find(|entry| entry.scene == scene);
        assert!(recorded.is_some(), "no recorded scene {scene}");
        let recorded = recorded.unwrap_or_default();
        let writes: Vec<String> = self
            .terminal
            .writes()
            .into_iter()
            .filter(|write| write != "\x1b[?25l" && write != "\x1b[?25h")
            .collect();
        assert_eq!(writes, recorded.writes, "{scene}");
    }
}

/// The writes a scene must produce.
#[derive(Default, Deserialize)]
struct RecordedScene {
    /// Identifies the scene.
    scene: String,
    /// Writes in order, without cursor visibility escapes.
    writes: Vec<String>,
}

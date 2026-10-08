//! Component capabilities, overlay records, the ordered container and the retained frame writer.
#![doc = include_str!("../../../../docs/terminal/rendering.md")]

mod component;
mod container;
mod diagnostics;
mod drawing;
mod input;
mod overlay_types;
mod runtime;
mod schedule;
mod screen;

use std::cell::{Cell, Ref, RefCell};
use std::io;
use std::rc::Rc;

use crate::images::terminal_image::TerminalImage;
use input::Input;
use runtime::is_enabled;
use schedule::Schedule;
use screen::Screen;

pub use crate::text::utils::visible_width;
pub use component::{CURSOR_MARKER, Component, FocusFlag, Focusable, InputHandler, is_focusable};
pub use container::{ComponentHandle, Container};
pub use input::{InputListener, InputListenerResult};
pub use overlay_types::{
    OverlayAnchor, OverlayHandle, OverlayMargin, OverlayMarginValue, OverlayOptions, SizeValue,
};
pub use runtime::{LogContext, RenderCallback, RenderTimer, TerminalHandle, TuiRuntime};

/// Writes components to a terminal as retained frames.
///
/// A `TUI` is one shared owner: cloning it retains its identity and never copies a
/// screen.
#[derive(Clone)]
pub struct TUI {
    /// State shared by every clone.
    shared: Rc<Shared>,
}

/// Everything a [`TUI`] owns.
struct Shared {
    /// The terminal frames are written to.
    terminal: TerminalHandle,
    /// Time, deferred work, environment and log files.
    runtime: Rc<dyn TuiRuntime>,
    /// Image capability and cell state the components share.
    images: TerminalImage,
    /// The ordered components that make up the frame.
    container: RefCell<Container>,
    /// What the terminal currently shows.
    screen: RefCell<Screen>,
    /// Pending and stopped state of render requests.
    schedule: RefCell<Schedule>,
    /// Listeners, focus and the debug callback.
    input: RefCell<Input>,
    /// Whether the hardware cursor is shown where a component asks for it.
    show_hardware_cursor: Cell<bool>,
    /// Whether shrinking content clears the screen.
    clear_on_shrink: Cell<bool>,
    /// Whether components were invalidated since the running component callback began.
    invalidated: Cell<bool>,
}

impl TUI {
    /// Creates a writer for `terminal` that has drawn nothing.
    ///
    /// `show_hardware_cursor` overrides the `MAESTRO_HARDWARE_CURSOR` environment
    /// default; clear-on-shrink defaults to `MAESTRO_CLEAR_ON_SHRINK`. Both are on only
    /// when the variable is exactly `1`.
    #[must_use]
    pub fn new(
        terminal: TerminalHandle,
        runtime: Rc<dyn TuiRuntime>,
        images: TerminalImage,
        show_hardware_cursor: Option<bool>,
    ) -> Self {
        let show_hardware_cursor = show_hardware_cursor
            .unwrap_or_else(|| is_enabled(&*runtime, "MAESTRO_HARDWARE_CURSOR"));
        let clear_on_shrink = is_enabled(&*runtime, "MAESTRO_CLEAR_ON_SHRINK");
        Self {
            shared: Rc::new(Shared {
                terminal,
                runtime,
                images,
                container: RefCell::new(Container::new()),
                screen: RefCell::new(Screen::default()),
                schedule: RefCell::new(Schedule::default()),
                input: RefCell::new(Input::default()),
                show_hardware_cursor: Cell::new(show_hardware_cursor),
                clear_on_shrink: Cell::new(clear_on_shrink),
                invalidated: Cell::new(false),
            }),
        }
    }

    /// The terminal frames are written to.
    #[must_use]
    pub fn terminal(&self) -> &TerminalHandle {
        &self.shared.terminal
    }

    /// The components in render order.
    #[must_use]
    pub fn children(&self) -> Ref<'_, [ComponentHandle]> {
        Ref::map(self.shared.container.borrow(), |container| {
            container.children.as_slice()
        })
    }

    /// Appends a component.
    ///
    /// Unless the component is running, the writer also reads its focus flag, so the
    /// component can be flagged while it runs a callback.
    pub fn add_child(&self, component: ComponentHandle) {
        drop(self.focus_flag_of(&component));
        self.shared.container.borrow_mut().add_child(component);
    }

    /// Removes the first occurrence of `component`; a missing component is ignored.
    pub fn remove_child(&self, component: &ComponentHandle) {
        self.shared.container.borrow_mut().remove_child(component);
    }

    /// Removes every component.
    pub fn clear(&self) {
        self.shared.container.borrow_mut().clear();
    }

    /// Full redraws begun so far.
    #[must_use]
    pub fn full_redraws(&self) -> usize {
        self.shared.screen.borrow().full_redraws
    }

    /// Whether the hardware cursor is shown where a component asks for it.
    #[must_use]
    pub fn get_show_hardware_cursor(&self) -> bool {
        self.shared.show_hardware_cursor.get()
    }

    /// Chooses whether the hardware cursor is shown; a change requests a frame, and
    /// turning it off hides the cursor at once.
    ///
    /// # Errors
    ///
    /// Returns the terminal's error unchanged.
    pub fn set_show_hardware_cursor(&self, enabled: bool) -> io::Result<()> {
        if self.shared.show_hardware_cursor.replace(enabled) == enabled {
            return Ok(());
        }
        if !enabled {
            self.shared.terminal.borrow_mut().hide_cursor()?;
        }
        self.request_render(false);
        Ok(())
    }

    /// Whether shrinking content clears the screen.
    #[must_use]
    pub fn get_clear_on_shrink(&self) -> bool {
        self.shared.clear_on_shrink.get()
    }

    /// Chooses whether shrinking content clears the screen; this only changes the
    /// policy and requests no frame.
    pub fn set_clear_on_shrink(&self, enabled: bool) {
        self.shared.clear_on_shrink.set(enabled);
    }

    /// Drops the cached rendering of every component, walking the live list of children.
    ///
    /// A child that is running a callback is borrowed exclusively and cannot be invalidated
    /// inside it. When the writer started that callback, as it does for rendering and
    /// input, the component is invalidated as soon as the callback returns, before it
    /// renders again.
    pub fn invalidate(&self) {
        for child in self.live_children() {
            if let Ok(mut idle) = child.try_borrow_mut() {
                idle.invalidate();
            }
        }
        self.shared.invalidated.set(true);
    }

    /// Starts the terminal, hides the cursor, asks image terminals for their cell size and
    /// requests the first frame.
    ///
    /// # Errors
    ///
    /// Returns the terminal's error unchanged.
    pub fn start(&self) -> io::Result<()> {
        self.shared.schedule.borrow_mut().restart();
        self.shared
            .terminal
            .borrow_mut()
            .start(self.input_callback(), self.resize_callback())?;
        self.shared.terminal.borrow_mut().hide_cursor()?;
        self.query_cell_size()?;
        self.request_render(false);
        Ok(())
    }

    /// Cancels pending work, shows the cursor and stops the terminal.
    ///
    /// # Errors
    ///
    /// Returns the terminal's error unchanged.
    pub fn stop(&self) -> io::Result<()> {
        let pending = self.shared.schedule.borrow_mut().halt();
        for mut timer in pending.into_iter().flatten() {
            timer.cancel();
        }
        self.move_below_content()?;
        self.shared.terminal.borrow_mut().show_cursor()?;
        self.shared.terminal.borrow_mut().stop()
    }

    /// The terminal callback that requests a frame when the terminal is resized.
    fn resize_callback(&self) -> Box<dyn FnMut()> {
        let weak = Rc::downgrade(&self.shared);
        Box::new(move || {
            if let Some(shared) = weak.upgrade() {
                TUI { shared }.request_render(false);
            }
        })
    }

    /// The children by position, each fetched when the walk reaches it: a child added
    /// during the walk is visited and one removed before its turn is not.
    fn live_children(&self) -> impl Iterator<Item = ComponentHandle> + '_ {
        (0..).map_while(|index| self.shared.container.borrow().children.get(index).cloned())
    }

    /// Runs `callback` on `component`. When [`TUI::invalidate`] was called meanwhile it could
    /// not reach `component`, so `component` is invalidated once the callback returns.
    fn run_component<R>(
        &self,
        component: &ComponentHandle,
        callback: impl FnOnce(&mut dyn Component) -> R,
    ) -> R {
        self.shared.invalidated.set(false);
        let result = callback(&mut *component.borrow_mut());
        if self.shared.invalidated.replace(false) {
            component.borrow_mut().invalidate();
        }
        result
    }

    /// Renders every component for `width`, walking the live list of children.
    fn render_children(&self, width: usize) -> Vec<String> {
        self.live_children()
            .flat_map(|child| self.run_component(&child, |child| child.render(width)))
            .collect()
    }
}

impl Component for TUI {
    fn render(&mut self, width: usize) -> Vec<String> {
        self.render_children(width)
    }

    fn invalidate(&mut self) {
        TUI::invalidate(self);
    }
}

//! Component capabilities, retained overlays, the ordered container and the frame writer.
#![doc = include_str!("../../../../docs/terminal/rendering.md")]
#![doc = include_str!("../../../../docs/terminal/overlays.md")]

mod component;
mod container;
mod diagnostics;
mod drawing;
mod geometry;
mod input;
mod overlay_types;
mod overlays;
mod runtime;
mod schedule;
mod screen;

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

use crate::images::terminal_image::TerminalImage;
use input::Input;
use runtime::is_enabled;
use schedule::Schedule;
use screen::Screen;

pub use crate::text::utils::visible_width;
pub use component::{CURSOR_MARKER, Component, FocusFlag, Focusable, InputHandler, is_focusable};
pub use container::{ChildArray, ComponentHandle, Container};
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
    container: Container,
    /// What the terminal currently shows.
    screen: RefCell<Screen>,
    /// Pending and stopped state of render requests.
    schedule: RefCell<Schedule>,
    /// Listeners, focus and the debug callback.
    input: RefCell<Input>,
    /// Retained entries in creation order.
    overlays: RefCell<Vec<Rc<overlays::Entry>>>,
    /// Counter for visual focus order.
    focus_order: Cell<usize>,
    /// Overlay provenance retained while removal callbacks run.
    focused_overlay: RefCell<std::rc::Weak<overlays::Entry>>,
    /// Whether the hardware cursor is shown where a component asks for it.
    show_hardware_cursor: Cell<bool>,
    /// Whether shrinking content clears the screen.
    clear_on_shrink: Cell<bool>,
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
                container: Container::new(),
                screen: RefCell::new(Screen::default()),
                schedule: RefCell::new(Schedule::default()),
                input: RefCell::new(Input::default()),
                overlays: RefCell::default(),
                focus_order: Cell::new(0),
                focused_overlay: RefCell::default(),
                show_hardware_cursor: Cell::new(show_hardware_cursor),
                clear_on_shrink: Cell::new(clear_on_shrink),
            }),
        }
    }

    /// The host used for deferred writer effects.
    pub(crate) fn runtime(&self) -> Rc<dyn TuiRuntime> {
        Rc::clone(&self.shared.runtime)
    }

    /// An ordinary render request that does not retain the writer.
    pub(crate) fn weak_render_request(&self) -> Rc<dyn Fn()> {
        let weak = Rc::downgrade(&self.shared);
        Rc::new(move || {
            if let Some(shared) = weak.upgrade() {
                TUI { shared }.request_render(false);
            }
        })
    }

    /// The terminal frames are written to.
    #[must_use]
    pub fn terminal(&self) -> &TerminalHandle {
        &self.shared.terminal
    }

    /// The array of components in render order, shared with the writer.
    #[must_use]
    pub fn children(&self) -> ChildArray {
        self.shared.container.children()
    }

    /// Makes `children` the array of components the writer renders. The previous array is
    /// left to its holders, and a walk already running keeps it.
    pub fn set_children(&self, children: ChildArray) {
        self.shared.container.set_children(children);
    }

    /// Appends a component.
    pub fn add_child(&self, component: ComponentHandle) {
        self.shared.container.add_child(component);
    }

    /// Removes the first occurrence of `component`; a missing component is ignored.
    pub fn remove_child(&self, component: &ComponentHandle) {
        self.shared.container.remove_child(component);
    }

    /// Replaces the components with a new empty array. A walk already running keeps the
    /// array it started on, and a handle kept from `children` stays with the old array.
    pub fn clear(&self) {
        self.shared.container.clear();
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

    /// Drops the cached rendering of the components, walking the array of children held
    /// when the call starts.
    ///
    /// Each child the walk reaches is invalidated in order before the call returns, also
    /// one that is rendering or handling input at that moment, once per position the walk
    /// visits. A child added to the array is reached only if the walk gets to its position,
    /// a removal at or before the walk's position makes it skip the next child, and
    /// clearing the writer or assigning another array starts one the walk does not see.
    /// After base children, the live overlay stack is walked, including hidden entries.
    pub fn invalidate(&self) {
        self.shared.container.invalidate();
        for entry in (0..).map_while(|index| self.overlay_at(index)) {
            entry.component.invalidate();
        }
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
}

impl Component for TUI {
    fn render(&self, width: usize) -> Vec<String> {
        self.shared.container.render(width)
    }

    fn invalidate(&self) {
        TUI::invalidate(self);
    }
}

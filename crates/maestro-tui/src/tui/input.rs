//! Routing of terminal input: live listeners, cell-size replies, the debug key and focus.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use maestro_listeners::Listeners;

use crate::images::terminal_image::CellDimensions;
use crate::keys::{is_key_release, matches_key};

use super::overlays::Focus;
use super::{Component, ComponentHandle, Focusable, TUI};

/// What an input listener decided about one chunk of input.
pub enum InputListenerResult {
    /// Leave the input as it is for the next listener.
    Pass,
    /// Stop here; nothing receives the input.
    Consume,
    /// Pass this text on instead. Later listeners still see it, even when it is empty;
    /// text that is still empty after the last listener is delivered to nothing.
    Replace(String),
}

/// A function that sees every chunk of input before the focused component does.
pub type InputListener = Rc<dyn Fn(&str) -> InputListenerResult>;

/// Asks the terminal for the pixel size of a cell.
const CELL_SIZE_QUERY: &str = "\x1b[16t";

/// Key that runs the debug callback instead of reaching the focused component.
const DEBUG_KEY: &str = "shift+ctrl+d";

/// Listeners, focus and the debug callback.
#[derive(Default)]
pub(super) struct Input {
    /// Input listeners in registration order, shared with their removers.
    pub(super) listeners: Rc<RefCell<Listeners<InputListener>>>,
    /// The component that receives input.
    pub(super) focused: Focus,
    /// Called when the debug key is pressed.
    on_debug: Option<Rc<dyn Fn()>>,
}

/// A chunk of input read as a reply to the cell-size query.
enum CellReply {
    /// Not a reply; the chunk is ordinary input.
    Unrecognized,
    /// A reply whose size cannot be applied; the chunk is dropped.
    Unusable,
    /// A reply carrying a size.
    Size(CellDimensions),
}

/// The focus capability of `component`.
fn focusable(component: &dyn Component) -> Option<&dyn Focusable> {
    component.focusable()
}

/// Reads `data` as `ESC [ 6 ; height ; width t`, the whole chunk and ASCII digits only.
fn parse_cell_reply(data: &str) -> CellReply {
    let Some(body) = data
        .strip_prefix("\x1b[6;")
        .and_then(|rest| rest.strip_suffix('t'))
    else {
        return CellReply::Unrecognized;
    };
    let Some((height, width)) = body.split_once(';') else {
        return CellReply::Unrecognized;
    };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(height) || !digits(width) {
        return CellReply::Unrecognized;
    }
    match (height.parse::<u32>(), width.parse::<u32>()) {
        (Ok(height_px), Ok(width_px)) if height_px > 0 && width_px > 0 => {
            CellReply::Size(CellDimensions {
                width_px,
                height_px,
            })
        }
        _ => CellReply::Unusable,
    }
}

impl TUI {
    /// Moves keyboard focus at once: the previous component loses its focus flag, the new
    /// one gains it, and a frame is not requested. A component that cannot hold focus is
    /// not flagged.
    pub fn set_focus(&self, component: Option<ComponentHandle>) {
        let capture = self
            .shared
            .overlays
            .borrow()
            .iter()
            .find(|entry| {
                component
                    .as_ref()
                    .is_some_and(|focus| Rc::ptr_eq(focus, &entry.capture.component))
            })
            .map(|entry| Rc::clone(&entry.capture));
        self.commit_focus(capture.map_or(Focus::Base(component), Focus::Captured));
    }

    /// Commits every focus change through the same flag and recipient assignment.
    pub(super) fn commit_focus(&self, focus: Focus) {
        let previous = std::mem::take(&mut self.shared.input.borrow_mut().focused);
        if let Some(flag) = previous.component().map(AsRef::as_ref).and_then(focusable) {
            flag.set_focused(false);
        }
        let component = focus.component().cloned();
        self.shared.input.borrow_mut().focused = focus;
        if let Some(flag) = component.as_deref().and_then(focusable) {
            flag.set_focused(true);
        }
    }

    /// Sets the callback that runs when the debug key is pressed, replacing any earlier one.
    pub fn set_on_debug(&self, callback: Option<Rc<dyn Fn()>>) {
        let replaced = std::mem::replace(&mut self.shared.input.borrow_mut().on_debug, callback);
        drop(replaced);
    }

    /// Adds a listener unless the same one is present; the returned function removes it
    /// and may be called again harmlessly. Dropping that function does not remove it.
    pub fn add_input_listener(&self, listener: InputListener) -> Box<dyn Fn()> {
        let listeners = Rc::clone(&self.shared.input.borrow().listeners);
        Box::new(Listeners::subscribe(&listeners, listener))
    }

    /// Removes a listener; one that is not present is ignored.
    pub fn remove_input_listener(&self, listener: &InputListener) {
        let listeners = Rc::clone(&self.shared.input.borrow().listeners);
        let removed = listeners.borrow_mut().remove(Rc::as_ptr(listener));
        drop(removed);
    }

    /// Asks the terminal for its cell size, which only image rendering uses, so terminals
    /// without image support are not asked.
    ///
    /// # Errors
    ///
    /// Returns the terminal's error unchanged.
    pub(super) fn query_cell_size(&self) -> io::Result<()> {
        if self.shared.images.get_capabilities().images.is_none() {
            return Ok(());
        }
        self.shared.terminal.borrow_mut().write(CELL_SIZE_QUERY)
    }

    /// The terminal callback that routes input.
    pub(super) fn input_callback(&self) -> Box<dyn FnMut(&str)> {
        let weak = Rc::downgrade(&self.shared);
        Box::new(move |data| {
            let Some(shared) = weak.upgrade() else {
                return;
            };
            let tui = TUI { shared };
            if let Err(error) = tui.handle_input(data) {
                tui.shared
                    .runtime
                    .spawn_local(Box::pin(std::future::ready(Err(error))));
            }
        })
    }

    /// Routes one chunk of input: listeners first, then cell-size replies, the debug key
    /// and finally the focused component.
    fn handle_input(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        let Some(data) = self.run_listeners(data) else {
            return Ok(());
        };
        if self.consume_cell_size_reply(&data) {
            return Ok(());
        }
        if matches_key(&data, DEBUG_KEY) {
            let on_debug = self.shared.input.borrow().on_debug.clone();
            if let Some(on_debug) = on_debug {
                on_debug();
                return Ok(());
            }
        }
        self.repair_overlay_focus();
        self.forward_to_focus(&data)
    }

    /// Lets each listener, in registration order, consume or rewrite `data`. The
    /// collection is walked live: a listener added meanwhile is visited, one removed
    /// before it is reached is skipped. Returns the text to route on, or `None` when it
    /// was consumed or rewritten to nothing.
    fn run_listeners(&self, data: &str) -> Option<String> {
        let listeners = Rc::clone(&self.shared.input.borrow().listeners);
        let mut current = data.to_owned();
        let mut visited = false;
        let outcome = Listeners::try_for_each_live(&listeners, |listener| {
            visited = true;
            match listener(&current) {
                InputListenerResult::Pass => Ok(()),
                InputListenerResult::Consume => Err(()),
                InputListenerResult::Replace(text) => {
                    current = text;
                    Ok(())
                }
            }
        });
        if !visited {
            return Some(current);
        }
        (outcome.is_ok() && !current.is_empty()).then_some(current)
    }

    /// Applies a cell-size reply to the shared image state; true when `data` was one.
    fn consume_cell_size_reply(&self, data: &str) -> bool {
        match parse_cell_reply(data) {
            CellReply::Unrecognized => false,
            CellReply::Unusable => true,
            CellReply::Size(cells) => {
                self.shared.images.set_cell_dimensions(cells);
                self.invalidate();
                self.request_render(false);
                true
            }
        }
    }

    /// Gives `data` to the focused component when it takes input, dropping key releases it
    /// did not ask for, then requests a frame.
    fn forward_to_focus(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        let focused = self.shared.input.borrow().focused.component().cloned();
        let Some(focused) = focused else {
            return Ok(());
        };
        let Some(handler) = focused.input_handler() else {
            return Ok(());
        };
        if is_key_release(data) && !focused.wants_key_release() {
            return Ok(());
        }
        handler.try_handle_input(data)?;
        self.request_render(false);
        Ok(())
    }
}

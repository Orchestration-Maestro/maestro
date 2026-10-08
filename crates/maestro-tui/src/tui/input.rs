//! Routing of terminal input: live listeners, cell-size replies, the debug key and focus.

use std::cell::RefCell;
use std::io;
use std::rc::{Rc, Weak};

use crate::images::terminal_image::CellDimensions;
use crate::keys::{is_key_release, matches_key};

use super::{Component, ComponentHandle, FocusFlag, TUI};

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
    /// Listeners in insertion order; a removed one leaves a gap until no dispatch is
    /// walking the list, so indexes stay valid.
    listeners: Vec<Option<InputListener>>,
    /// How many dispatches are walking `listeners`.
    dispatching: usize,
    /// The component that receives input.
    focused: Option<Focus>,
    /// Called when the debug key is pressed.
    on_debug: Option<Rc<dyn Fn()>>,
    /// The focus flags read from components, so focus can move while a component runs.
    flags: Vec<(Weak<RefCell<dyn Component>>, FocusFlag)>,
}

/// The component that receives input and the flag that shows it has focus.
struct Focus {
    /// The component.
    component: ComponentHandle,
    /// Its flag, when it can hold focus and the flag was reachable.
    flag: Option<FocusFlag>,
}

/// One position of the listener list.
enum Slot {
    /// Past the last position.
    End,
    /// A listener that was removed.
    Removed,
    /// A listener to run.
    Live(InputListener),
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

impl Input {
    /// Appends `listener` unless the same one is already present.
    fn add(&mut self, listener: &InputListener) {
        let present = self
            .listeners
            .iter()
            .flatten()
            .any(|existing| Rc::ptr_eq(existing, listener));
        if !present {
            self.listeners.push(Some(Rc::clone(listener)));
        }
    }

    /// Removes `listener` and hands it back so it is dropped without a borrow held.
    fn remove(&mut self, listener: &InputListener) -> Option<InputListener> {
        let slot = self.listeners.iter_mut().find(|slot| {
            slot.as_ref()
                .is_some_and(|existing| Rc::ptr_eq(existing, listener))
        })?;
        let removed = slot.take();
        if self.dispatching == 0 {
            self.listeners.retain(Option::is_some);
        }
        removed
    }

    /// The flag already read from `component`, forgetting the flags of dropped components.
    fn known_flag(&mut self, component: &ComponentHandle) -> Option<FocusFlag> {
        self.flags.retain(|(owner, _)| owner.strong_count() > 0);
        let component = Rc::downgrade(component);
        self.flags
            .iter()
            .find(|(owner, _)| Weak::ptr_eq(owner, &component))
            .map(|(_, flag)| flag.clone())
    }

    /// The position `index` of the listener list.
    fn slot(&self, index: usize) -> Slot {
        match self.listeners.get(index) {
            None => Slot::End,
            Some(None) => Slot::Removed,
            Some(Some(listener)) => Slot::Live(Rc::clone(listener)),
        }
    }

    /// Ends one dispatch and closes the gaps once none is left.
    fn finish_dispatch(&mut self) {
        self.dispatching -= 1;
        if self.dispatching == 0 {
            self.listeners.retain(Option::is_some);
        }
    }
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
    /// The focus flag of `component`: the one read from it earlier, else the one it offers
    /// now. `None` when the component cannot hold focus, or when it is running a callback and
    /// no flag was read from it earlier.
    pub(super) fn focus_flag_of(&self, component: &ComponentHandle) -> Option<FocusFlag> {
        let known = self.shared.input.borrow_mut().known_flag(component);
        if known.is_some() {
            return known;
        }
        let flag = component
            .try_borrow()
            .ok()?
            .focusable()?
            .focus_flag()
            .clone();
        let owner = Rc::downgrade(component);
        self.shared
            .input
            .borrow_mut()
            .flags
            .push((owner, flag.clone()));
        Some(flag)
    }

    /// Moves keyboard focus at once: the previous component loses its focus flag, the new
    /// one gains it, and a frame is not requested.
    ///
    /// The writer reads a component's flag when it is added with [`TUI::add_child`] or given
    /// focus, provided it is not running then. A running component whose flag was never
    /// read cannot be flagged; it still receives input once it is the focus.
    pub fn set_focus(&self, component: Option<ComponentHandle>) {
        let next = component.map(|component| Focus {
            flag: self.focus_flag_of(&component),
            component,
        });
        let next_flag = next.as_ref().and_then(|focus| focus.flag.clone());
        let previous = std::mem::replace(&mut self.shared.input.borrow_mut().focused, next);
        if let Some(flag) = previous.and_then(|focus| focus.flag) {
            flag.set(false);
        }
        if let Some(flag) = next_flag {
            flag.set(true);
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
        self.shared.input.borrow_mut().add(&listener);
        let weak = Rc::downgrade(&self.shared);
        Box::new(move || {
            if let Some(shared) = weak.upgrade() {
                TUI { shared }.remove_input_listener(&listener);
            }
        })
    }

    /// Removes a listener; one that is not present is ignored.
    pub fn remove_input_listener(&self, listener: &InputListener) {
        let removed = self.shared.input.borrow_mut().remove(listener);
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
            if let Some(shared) = weak.upgrade() {
                TUI { shared }.handle_input(data);
            }
        })
    }

    /// Routes one chunk of input: listeners first, then cell-size replies, the debug key
    /// and finally the focused component.
    fn handle_input(&self, data: &str) {
        let Some(data) = self.run_listeners(data) else {
            return;
        };
        if self.consume_cell_size_reply(&data) {
            return;
        }
        if matches_key(&data, DEBUG_KEY) {
            let on_debug = self.shared.input.borrow().on_debug.clone();
            if let Some(on_debug) = on_debug {
                on_debug();
                return;
            }
        }
        self.forward_to_focus(&data);
    }

    /// Lets each listener, in insertion order, consume or rewrite `data`. The list is
    /// walked live: a listener added meanwhile is visited, one removed is skipped.
    /// Returns the text to route on, or `None` when it was consumed or rewritten to
    /// nothing.
    fn run_listeners(&self, data: &str) -> Option<String> {
        if self
            .shared
            .input
            .borrow()
            .listeners
            .iter()
            .all(Option::is_none)
        {
            return Some(data.to_owned());
        }
        self.shared.input.borrow_mut().dispatching += 1;
        let mut current = data.to_owned();
        let mut consumed = false;
        let mut index = 0;
        loop {
            let slot = self.shared.input.borrow().slot(index);
            match slot {
                Slot::End => break,
                Slot::Removed => {}
                Slot::Live(listener) => match listener(&current) {
                    InputListenerResult::Pass => {}
                    InputListenerResult::Consume => {
                        consumed = true;
                        break;
                    }
                    InputListenerResult::Replace(text) => current = text,
                },
            }
            index += 1;
        }
        self.shared.input.borrow_mut().finish_dispatch();
        (!consumed && !current.is_empty()).then_some(current)
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
    fn forward_to_focus(&self, data: &str) {
        let focused = self
            .shared
            .input
            .borrow()
            .focused
            .as_ref()
            .map(|focus| Rc::clone(&focus.component));
        let Some(focused) = focused else {
            return;
        };
        let delivered = self.run_component(&focused, |component| {
            if component.input_handler().is_none() {
                return false;
            }
            if is_key_release(data) && !component.wants_key_release() {
                return false;
            }
            if let Some(handler) = component.input_handler() {
                handler.handle_input(data);
            }
            true
        });
        if delivered {
            self.request_render(false);
        }
    }
}

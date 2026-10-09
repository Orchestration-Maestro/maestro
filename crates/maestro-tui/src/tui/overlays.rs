//! Retained overlay identities and their controls.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;

use super::{ComponentHandle, OverlayHandle, OverlayOptions, TUI};

/// A creation-time focus target; overlay links always point to older entries.
#[derive(Clone)]
enum Predecessor {
    /// Focus outside the overlay stack, possibly absent.
    Base(Option<ComponentHandle>),
    /// An earlier-created overlay.
    Overlay(Rc<Entry>),
}

/// One identity retained by the owner and its control handle.
pub(super) struct Entry {
    /// The rendered component.
    pub component: ComponentHandle,
    /// Options shared with the caller.
    pub options: Rc<RefCell<OverlayOptions>>,
    /// Creation-time focus target, relinked when an ancestor is removed.
    predecessor: RefCell<Predecessor>,
    /// Temporary hiding, independent of callback visibility.
    hidden: Cell<bool>,
    /// Visual order, independent of stack order.
    pub order: Cell<usize>,
}

/// Controls one entry while keeping its owner alive.
struct Handle {
    /// Owner shared with every writer clone.
    tui: TUI,
    /// The controlled identity, retained even after removal.
    entry: Rc<Entry>,
}

impl Entry {
    /// Whether showing this entry automatically captures focus.
    pub(super) fn capturing(&self) -> bool {
        self.options.borrow().non_capturing != Some(true)
    }
}

impl TUI {
    /// Shows `component` with caller-editable options, capturing focus when visible
    /// unless `non_capturing` is true. Dropping the returned handle leaves it shown.
    ///
    /// # Errors
    /// Returns cursor-hiding errors after the stack and focus changes; no frame is
    /// requested on that failure.
    pub fn show_overlay(
        &self,
        component: ComponentHandle,
        options: Option<Rc<RefCell<OverlayOptions>>>,
    ) -> io::Result<Box<dyn OverlayHandle>> {
        let focused = self.focused_component();
        let predecessor = self
            .shared
            .overlays
            .borrow()
            .iter()
            .rev()
            .find(|entry| {
                focused
                    .as_ref()
                    .is_some_and(|focus| Rc::ptr_eq(focus, &entry.component))
            })
            .map_or_else(
                || Predecessor::Base(focused.clone()),
                |entry| Predecessor::Overlay(Rc::clone(entry)),
            );
        let entry = Rc::new(Entry {
            component,
            options: options.unwrap_or_default(),
            predecessor: RefCell::new(predecessor),
            hidden: Cell::new(false),
            order: Cell::new(self.next_focus_order()),
        });
        self.shared.overlays.borrow_mut().push(Rc::clone(&entry));
        if entry.capturing() && self.overlay_visible(&entry) {
            self.set_focus(Some(Rc::clone(&entry.component)));
        }
        self.shared.terminal.borrow_mut().hide_cursor()?;
        self.request_render(false);
        Ok(Box::new(Handle {
            tui: self.clone(),
            entry,
        }))
    }

    /// Removes the last-created overlay; an empty stack does nothing.
    ///
    /// # Errors
    /// Returns cursor-hiding errors after removal and focus restoration, without
    /// requesting a frame on that failure.
    pub fn hide_overlay(&self) -> io::Result<()> {
        let entry = self.shared.overlays.borrow().last().cloned();
        if let Some(entry) = entry {
            self.remove_overlay(&entry)?;
        }
        Ok(())
    }

    /// Whether a currently visible entry exists, including noncapturing entries.
    #[must_use]
    pub fn has_overlay(&self) -> bool {
        let length = self.shared.overlays.borrow().len();
        (0..length).any(|index| {
            self.overlay_at(index)
                .is_some_and(|entry| self.overlay_visible(&entry))
        })
    }

    /// The entry currently at `index`, retaining it without a stack borrow.
    pub(super) fn overlay_at(&self, index: usize) -> Option<Rc<Entry>> {
        self.shared.overlays.borrow().get(index).cloned()
    }

    /// Whether the entry is still in the stack.
    fn attached(&self, entry: &Rc<Entry>) -> bool {
        self.shared
            .overlays
            .borrow()
            .iter()
            .any(|item| Rc::ptr_eq(item, entry))
    }

    /// Visibility at current terminal dimensions; hidden entries skip the callback.
    pub(super) fn overlay_visible(&self, entry: &Entry) -> bool {
        if entry.hidden.get() {
            return false;
        }
        let visible = entry.options.borrow().visible.clone();
        visible.is_none_or(|visible| {
            let (width, height) = {
                let terminal = self.shared.terminal.borrow();
                (terminal.columns(), terminal.rows())
            };
            visible(width, height)
        })
    }

    /// The last-created visible capturing entry, reading current slots while walking.
    fn top_capturing(&self) -> Option<Rc<Entry>> {
        let length = self.shared.overlays.borrow().len();
        (0..length).rev().find_map(|index| {
            self.overlay_at(index)
                .filter(|entry| entry.capturing() && self.overlay_visible(entry))
        })
    }

    /// The next visual order.
    fn next_focus_order(&self) -> usize {
        let order = self.shared.focus_order.get().saturating_add(1);
        self.shared.focus_order.set(order);
        order
    }

    /// Whether this component currently owns focus.
    fn overlay_focused(&self, entry: &Entry) -> bool {
        self.focused_component()
            .is_some_and(|focus| Rc::ptr_eq(&focus, &entry.component))
    }

    /// Restores a capturing target, or walks older predecessors to an available target.
    fn restore_overlay_focus(&self, entry: &Rc<Entry>) {
        if let Some(top) = self.top_capturing().filter(|top| !Rc::ptr_eq(top, entry)) {
            self.set_focus(Some(Rc::clone(&top.component)));
            return;
        }
        let mut predecessor = entry.predecessor.borrow().clone();
        loop {
            match predecessor {
                Predecessor::Base(component) => {
                    self.set_focus(component);
                    return;
                }
                Predecessor::Overlay(previous)
                    if self.attached(&previous) && self.overlay_visible(&previous) =>
                {
                    self.set_focus(Some(Rc::clone(&previous.component)));
                    return;
                }
                Predecessor::Overlay(previous) => {
                    predecessor = previous.predecessor.borrow().clone();
                }
            }
        }
    }

    /// Removes and relinks one identity, completing state changes before terminal effects.
    fn remove_overlay(&self, entry: &Rc<Entry>) -> io::Result<()> {
        let removed = {
            let mut stack = self.shared.overlays.borrow_mut();
            stack
                .iter()
                .position(|item| Rc::ptr_eq(item, entry))
                .map(|index| stack.remove(index))
        };
        if removed.is_none() {
            return Ok(());
        }
        let predecessor = entry.predecessor.borrow().clone();
        let length = self.shared.overlays.borrow().len();
        for other in (0..length).filter_map(|index| self.overlay_at(index)) {
            let matches = matches!(&*other.predecessor.borrow(), Predecessor::Overlay(previous) if Rc::ptr_eq(previous, entry));
            if matches {
                let replaced = other.predecessor.replace(predecessor.clone());
                drop(replaced);
            }
        }
        if self.overlay_focused(entry) {
            self.restore_overlay_focus(entry);
        }
        if self.shared.overlays.borrow().is_empty() {
            self.shared.terminal.borrow_mut().hide_cursor()?;
        }
        self.request_render(false);
        Ok(())
    }

    /// Repairs focus just before ordinary input dispatch.
    pub(super) fn repair_overlay_focus(&self) {
        let focused = self.focused_component();
        let entry = self
            .shared
            .overlays
            .borrow()
            .iter()
            .find(|entry| {
                focused
                    .as_ref()
                    .is_some_and(|focus| Rc::ptr_eq(focus, &entry.component))
            })
            .cloned();
        if let Some(entry) = entry.filter(|entry| !self.overlay_visible(entry)) {
            self.restore_overlay_focus(&entry);
        }
    }
}

impl OverlayHandle for Handle {
    fn hide(&mut self) -> io::Result<()> {
        self.tui.remove_overlay(&self.entry)
    }

    fn set_hidden(&mut self, hidden: bool) {
        if !self.tui.attached(&self.entry) || self.entry.hidden.replace(hidden) == hidden {
            return;
        }
        if hidden {
            if self.tui.overlay_focused(&self.entry) {
                self.tui.restore_overlay_focus(&self.entry);
            }
        } else if self.entry.capturing() && self.tui.overlay_visible(&self.entry) {
            self.entry.order.set(self.tui.next_focus_order());
            self.tui.set_focus(Some(Rc::clone(&self.entry.component)));
        }
        self.tui.request_render(false);
    }

    fn is_hidden(&self) -> bool {
        self.entry.hidden.get()
    }

    fn focus(&mut self) {
        if !self.tui.attached(&self.entry) || !self.tui.overlay_visible(&self.entry) {
            return;
        }
        if !self.is_focused() {
            self.tui.set_focus(Some(Rc::clone(&self.entry.component)));
        }
        self.entry.order.set(self.tui.next_focus_order());
        self.tui.request_render(false);
    }

    fn unfocus(&mut self) {
        if !self.tui.attached(&self.entry) || !self.is_focused() {
            return;
        }
        self.tui.restore_overlay_focus(&self.entry);
        self.tui.request_render(false);
    }

    fn is_focused(&self) -> bool {
        self.tui.overlay_focused(&self.entry)
    }
}

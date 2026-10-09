# Retained overlays

`TUI::show_overlay` adds a component to the writer's shared stack. Its handle keeps
that writer alive; dropping the handle does not remove the entry. Options are
shared with the caller and read again on subsequent operations.

```rust
use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use maestro_tui::{Component, OverlayAnchor, OverlayOptions, SizeValue, TUI};

/// Text displayed by the overlay.
struct Notice;
impl Component for Notice {
    fn render(&self, _width: usize) -> Vec<String> {
        vec!["Ready".to_owned()]
    }
}

/// Shows a notice using the caller's writer and terminal runtime.
fn show_notice(tui: &TUI) -> io::Result<()> {
    let options = Rc::new(RefCell::new(OverlayOptions {
        width: Some(SizeValue::Percentage("50%".to_owned())),
        anchor: Some(OverlayAnchor::TopRight),
        non_capturing: Some(true),
        ..OverlayOptions::default()
    }));
    let mut handle = tui.show_overlay(Rc::new(Notice), Some(options.clone()))?;
    options.borrow_mut().offset_y = Some(1);
    tui.request_render(false);
    handle.focus();
    handle.unfocus();
    handle.set_hidden(true);
    handle.set_hidden(false);
    handle.hide()?;
    Ok(())
}
# fn main() {}
```

## Placement

Width defaults to the smaller of 80 cells and available width. Minimum width is
applied before clamping to available width, whose floor is one cell. Negative
margins resolve to zero. Valid maximum height clamps to available height, also
with a one-row floor; absent or invalid maximum height leaves content untruncated.

Percentage syntax is ASCII digits, an optional decimal fraction and `%`, with no
whitespace or sign. Size percentages use terminal dimensions. Position percentages
use the remaining span after margins and content. An explicit row or column wins
over that axis's anchor; an invalid percentage position uses center on that axis.
Percentage coordinates retain floating-point precision through margins, offsets
and final clamping, converting to cells last. Oversized margins can place content
outside the terminal; they do not promise an onscreen rectangle.

Visible entries are selected from current slots up to the stack's starting length,
then rendered in stable focus order. The first layout determines rendering width
and height truncation; options edited by a render callback affect the second
layout's placement. Removing an already selected entry does not cancel that
render. [Text helpers](text.md) own column selection; composition expands tabs to
three spaces in displayed text before selecting both operands, preserves tabs in
recognized escape payloads and leaves image base lines unchanged.

## Focus and controls

Creation and unhide capture focus only when their capture gate permits it and
the retained component has an accepted availability observation. Noncapturing
creation/unhide does not itself capture focus. Explicit handle focus raises visual
order after a successful availability check. Removal by `hide_overlay` uses
creation order, not visual order. Public `set_focus` accepts the caller's selected
component without an overlay availability check.

When its component owns focus, removal or temporary hiding selects through the
top-candidate walk, otherwise its captured focus. The walk checks capture flags
before callbacks and rereads the accepted slot afterward; a replacement need not
be capturing. An accepted vacated slot ends the walk. Only unfocus excludes its
own selected entry.

A captured target is a component, not one showing of it. Resolution accepts a
live visible alias or advances through immutable captured predecessors to external
focus or none. Additional availability reads use a finite snapshot of matching
entries; side-effecting predicates can have additional effects at these validation
boundaries. An alias created inside such a predicate is outside that snapshot.
Acceptance requires a true observation and attachment/not-hidden state after the
callback, not a fixed point or a timeless visibility promise.

Input repair uses the first matching entry's visibility result. On false it
selects through the top walk or captured fallback; that walk may accept the same
entry on a second visibility call. It does not repair again after input handling.
Mutating controls on removed entries do nothing. Getters read stored hidden state
or current component identity, so a detached handle can report focused when a
live alias owns focus.

Hidden entries skip visibility callbacks. `has_overlay` checks current slots up
to the starting length and stops at the first accepted visibility result, including
a noncapturing entry. A callback can remove that entry before the query returns.
Invalidation visits base children first, then the live overlay
stack including hidden entries. [Frame rendering](rendering.md) owns input
routing, render scheduling and cursor placement.

## Terminal errors

Showing appends the entry and performs any applicable focus change before hiding
the cursor. Removing an attached entry performs any applicable focus restoration
before hiding the cursor if the stack is empty.
These operations return a terminal error unchanged and do not request a frame
when cursor hiding fails; completed state changes remain in place. Empty-stack
removal has no terminal effect.

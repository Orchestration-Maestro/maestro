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
Offsets apply before final margin clamping. Oversized margins can place content
outside the terminal; they do not promise an onscreen rectangle.

Visible entries are selected from current slots up to the stack's starting length,
then rendered in stable focus order. The first layout determines rendering width
and height truncation; options edited by a render callback affect the second
layout's placement. Removing an already selected entry does not cancel that
render. [Text helpers](text.md) own column selection; composition expands tabs to
three spaces in displayed text before selecting both operands, preserves tabs in
recognized escape payloads and leaves image base lines unchanged.

## Focus and controls

Capturing entries take focus on creation or unhide only when their visibility
check accepts them and leaves them attached and not hidden.
Noncapturing entries do not capture on creation or unhide; explicit focus and
predecessor restoration can select them. Explicit focus raises
visual order, but neither removal by `hide_overlay` nor fallback selection uses
that order: both use creation order.

Temporary hiding and removal are distinct. Repeated hidden flags and detached
mutations do nothing; `is_hidden` reports only the stored temporary flag.
`is_focused` compares component identity, not labels or attachment.
Restoration walks capturing candidates in reverse creation order and reads the
selected stack slot again after its visibility callback. Unfocusing the selected
entry instead falls directly to its captured predecessor. Predecessor
links skip removed, hidden and callback-invisible entries, reaching an available
component or the captured base focus, which can be absent. An overlay created while
the removed entry still owns focus captures that entry as an overlay predecessor,
not base focus.

Hidden entries skip visibility callbacks. `has_overlay` checks current slots up
to the starting length and stops at the first accepted visibility result, including
a noncapturing entry. A callback can remove that entry before the query returns.
Invalidation visits base children first, then the live overlay
stack including hidden entries. [Frame rendering](rendering.md) owns input
routing, render scheduling and cursor placement.

## Terminal errors

Showing an overlay updates stack and focus before hiding the cursor. Removing
one updates stack and focus before hiding the cursor when the stack becomes empty.
These operations return a terminal error unchanged and do not request a frame
when cursor hiding fails; completed state changes remain in place. Empty-stack
removal has no terminal effect.

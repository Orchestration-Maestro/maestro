//! The emulated terminal exposes what retained-frame scenarios observe.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod recording_terminal;
    pub mod virtual_terminal;
}
use support::virtual_terminal::VirtualTerminal;

#[test]
fn virtual_terminal_exposes_screen_history_and_callbacks() {
    let terminal = VirtualTerminal::new(5, 2);
    let inputs = Rc::new(RefCell::new(Vec::new()));
    let resizes = Rc::new(Cell::new(0));
    let port = terminal.handle();
    let input_log = Rc::clone(&inputs);
    let resize_count = Rc::clone(&resizes);
    port.borrow_mut()
        .start(
            Box::new(move |data| input_log.borrow_mut().push(data.to_owned())),
            Box::new(move || resize_count.set(resize_count.get() + 1)),
        )
        .unwrap();

    terminal.send_input("a");
    port.borrow_mut().write("one\r\ntwo\r\nthree").unwrap();
    assert_eq!(terminal.viewport(), ["two", "three"]);
    assert_eq!(terminal.history(), ["one", "two", "three"]);
    assert_eq!(
        terminal.cursor(),
        (5, 1),
        "a pending wrap leaves the cursor on the column count"
    );

    terminal.resize(6, 3);
    assert_eq!(resizes.get(), 1);
    assert_eq!((port.borrow().columns(), port.borrow().rows()), (6, 3));
    let resized_viewport = terminal.viewport();
    assert_eq!(resized_viewport.len(), 3);
    assert!(
        resized_viewport.iter().any(|row| row == "three"),
        "the content survives a resize: {resized_viewport:?}"
    );

    port.borrow_mut().stop().unwrap();
    terminal.send_input("ignored");
    terminal.resize(6, 3);
    assert_eq!(*inputs.borrow(), ["a"]);
    assert_eq!(resizes.get(), 1, "a stopped terminal reports nothing");
    assert!(port.borrow().kitty_protocol_active());
}

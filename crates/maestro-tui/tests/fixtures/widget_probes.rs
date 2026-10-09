//! Controlled styling functions for the widget tests.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Rows received by background functions, in call order.
pub type Calls = Rc<RefCell<Vec<String>>>;

/// A background function that wraps each row in the SGR background `color` read at call time
/// and records `label` followed by the row it received.
pub fn live_background(
    label: &'static str,
    color: &Rc<Cell<u8>>,
    calls: &Calls,
) -> Rc<dyn Fn(&str) -> String> {
    let (color, calls) = (Rc::clone(color), Rc::clone(calls));
    Rc::new(move |row| {
        calls.borrow_mut().push(format!("{label}{row}"));
        format!("\x1b[{}m{row}\x1b[49m", color.get())
    })
}

/// A background function with a fixed SGR background `color`.
pub fn background(label: &'static str, color: u8, calls: &Calls) -> Rc<dyn Fn(&str) -> String> {
    live_background(label, &Rc::new(Cell::new(color)), calls)
}

/// `rows` wrapped in the SGR background `color`.
pub fn tinted(color: u8, rows: &[&str]) -> Vec<String> {
    rows.iter()
        .map(|row| format!("\x1b[{color}m{row}\x1b[49m"))
        .collect()
}

//! Tests for the shared contract order.

use super::first_out_of_order;

#[test]
fn names_in_order_have_no_break() {
    assert_eq!(first_out_of_order(&["maestro-app", "maestro-text"]), None);
    assert_eq!(
        first_out_of_order(&["maestro-models", "maestro-models-catalog"]),
        None
    );
    assert_eq!(first_out_of_order(&[]), None);
}

#[test]
fn the_first_name_below_the_name_above_it_is_named() {
    assert_eq!(
        first_out_of_order(&["maestro-text", "maestro-app", "maestro-web"]),
        Some(1)
    );
    assert_eq!(
        first_out_of_order(&["maestro-agent", "maestro-tui", "maestro-text"]),
        Some(2)
    );
}

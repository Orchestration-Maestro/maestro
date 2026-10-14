//! Optional-field omission decisions.
use maestro_request::presence::{Omittable, Presence};

#[test]
fn omission_predicate_keeps_string_option_and_presence_rules() {
    assert!(!String::new().is_omitted());
    assert!(!String::from("value").is_omitted());
    assert!(Option::<bool>::None.is_omitted());
    assert!(!Some(false).is_omitted());
    assert!(!Some(serde_json::Value::Null).is_omitted());
    assert!(Presence::<bool>::Missing.is_omitted());
    assert!(!Presence::<bool>::Null.is_omitted());
    assert!(!Presence::Present(false).is_omitted());
}

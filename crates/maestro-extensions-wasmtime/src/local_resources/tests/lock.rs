//! A borrowed lock update ends with the pump that lent it.
use crate::local_resources::credential_scope::Endpoint;

#[test]
fn update_after_the_pump_ended_rejects_with_its_ended_text() {
    let (requests, pump) = tokio::sync::mpsc::unbounded_channel();
    drop(pump);
    let update = Endpoint { id: 0, requests };
    assert_eq!(
        update.update(None).unwrap_err(),
        "extension lock update ended"
    );
}

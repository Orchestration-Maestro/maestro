//! A keybindings manager ends with its capability.
use super::Pump;
use crate::tests::ended_errors;
use maestro_extensions::presentation::{ExtensionKeybindings, get_keybindings};

#[test]
fn released_keybindings_reject_every_kind_of_operation_with_their_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let keys = pump
        .registry()
        .keybindings(ExtensionKeybindings {
            manager: get_keybindings(),
            reload: None,
        })
        .unwrap();
    let lease = keys.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        [
            keys.matches("a".into(), "id".into()).unwrap_err(),
            keys.get_keys("id".into()).unwrap_err(),
            keys.get_definition("id".into()).unwrap_err(),
            keys.get_conflicts().err().unwrap(),
            keys.set_user_bindings(Vec::new()).unwrap_err(),
            keys.get_user_bindings().unwrap_err(),
            keys.get_resolved_bindings().unwrap_err(),
            keys.reload().unwrap_err(),
        ]
    });
    assert_eq!(errors, ["extension keybindings resource ended"; 8]);
}

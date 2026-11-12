//! A keybindings manager ends with its capability.
use super::Pump;
use crate::{
    keys_reads::{KeysEndpoint, KeysRequest},
    tests::ended_errors,
};
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
            read(&keys, |reply| {
                KeysRequest::Matches("a".into(), "id".into(), reply)
            }),
            read(&keys, |reply| KeysRequest::Keys("id".into(), reply)),
            read(&keys, |reply| KeysRequest::Definition("id".into(), reply)),
            read(&keys, KeysRequest::Conflicts),
            read(&keys, |reply| {
                KeysRequest::SetUserBindings(Vec::new(), reply)
            }),
            read(&keys, KeysRequest::UserBindings),
            read(&keys, KeysRequest::ResolvedBindings),
            read(&keys, KeysRequest::Reload),
        ]
    });
    assert_eq!(errors, ["extension keybindings resource ended"; 8]);
}

/// The failure one request on `keys` settles with.
fn read<T>(
    keys: &KeysEndpoint,
    request: impl FnOnce(crate::widget_reads::Reply<T>) -> KeysRequest,
) -> String {
    keys.read(request).err().unwrap()
}

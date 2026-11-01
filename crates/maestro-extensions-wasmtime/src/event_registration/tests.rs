//! Every shared event name registers the exact native event/result pair.
use super::{Binding, bindings};
use crate::lifecycle::replies::Replies;
use crate::{guest_calls::Callbacks, local_resources::Pump};
use maestro_extensions::{ExtensionAPI, ExtensionExecution, Local};
use serde::Serialize;

/// Emits a single exhaustive inventory test from the owning shared table.
macro_rules! inventory_test {
    (before { $($(#[$bm:meta])* $bn:ident($bt:ty),)* }
     session { $($(#[$sm:meta])* $sn:ident / $sd:ident($st:ty, $sdt:ty),)* }
     after { $($(#[$am:meta])* $an:ident($at:ty),)* }) => {
        /// Canonical names encoded by the same inventory's snake-case contract.
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        enum Name { $($(#[$bm])* $bn,)* $($(#[$sm])* $sd,)* $($(#[$am])* $an,)* }
        #[test]
        fn every_known_event_has_a_concrete_native_registration() {
            let (owner, api) = crate::tests::api::api(maestro_extensions::create_extension_runtime::<Local>());
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let (calls, _incoming) = tokio::sync::mpsc::unbounded_channel();
            let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
            let callback = callbacks.allocate().unwrap();
            let handler = callbacks.retain(callback.id, admission).unwrap();
            let mut count = 0;
            $(verify::<bindings::$bn>(&owner, &api, Name::$bn, &handler, &pump); count += 1;)*
            $(verify::<bindings::$sd>(&owner, &api, Name::$sd, &handler, &pump); count += 1;)*
            $(verify::<bindings::$an>(&owner, &api, Name::$an, &handler, &pump); count += 1;)*
            assert_eq!(owner.read().unwrap().handlers.len(), count);
            assert_eq!(count, 29);
        }
    };
}
maestro_extensions::extension_event_table!(inventory_test);

/// Checks the native owner's real downcast, including application-message specialization.
fn verify<B: Binding>(
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
    api: &ExtensionAPI<Local>,
    name: Name,
    handler: &crate::guest_calls::Handle,
    pump: &Pump,
) {
    let name = serde_json::to_value(name)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    super::register(api, name.clone(), handler.clone(), pump.registry()).unwrap();
    let owner = owner.read().unwrap();
    assert!(Local::recover_handler::<B::Event, B::Result>(&owner.handlers[&name][0]).is_some());
}

#[test]
fn provider_response_headers_cross_as_ordered_pairs() {
    use super::EventData;
    let mut event = maestro_extensions::AfterProviderResponseEvent {
        status: 200.0,
        headers: [("b", "2"), ("a", "1")]
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .into(),
    };
    let encoded: serde_json::Value =
        serde_json::from_str(&event.encode("after_provider_response").unwrap()).unwrap();
    assert_eq!(
        encoded,
        serde_json::json!({"status":200.0,"headers":[["a","1"],["b","2"]],"type":"after_provider_response"})
    );
    let pairs: maestro_extensions::AfterProviderResponseEvent<Vec<(String, String)>> =
        serde_json::from_value(encoded).unwrap();
    assert_eq!(
        pairs.headers,
        [
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "2".to_owned())
        ]
    );
    event
        .apply(
            r#"{"type":"after_provider_response","status":201.0,"headers":[["z","26"],["a","1"]]}"#,
        )
        .unwrap();
    assert_eq!(event.status.to_bits(), 201_f64.to_bits());
    let headers: Vec<_> = event
        .headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    assert_eq!(headers, [("a", "1"), ("z", "26")]);
}

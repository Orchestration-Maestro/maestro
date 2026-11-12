//! Overlay options end with their capability.
use super::Pump;
use crate::{
    local_resources::{Capability, overlays::Placement},
    overlay_reads::OptionsEndpoint,
    tests::ended_errors,
};
use maestro_extensions::{Local, create_extension_runtime};
use std::{cell::RefCell, rc::Rc};

#[test]
fn released_overlay_options_reject_every_kind_of_operation_with_their_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let (_owner, api) = crate::tests::api::api(create_extension_runtime::<Local>());
    let api = registry.api(api).unwrap();
    let placement = Placement {
        options: Rc::new(RefCell::new(crate::overlay_layout::decode("{}").unwrap())),
        reporter: Rc::default(),
    };
    let lease = registry
        .insert(Capability::OverlayOptions(placement))
        .unwrap();
    let options = OptionsEndpoint(lease.clone());
    let errors = ended_errors(lease, pump, admission, move || {
        [
            options.get().unwrap_err(),
            options.set("{}".into()).unwrap_err(),
            options.share().err().unwrap(),
            options.set_visible(api, None).unwrap_err(),
        ]
    });
    assert_eq!(errors, ["extension overlay options resource ended"; 4]);
}

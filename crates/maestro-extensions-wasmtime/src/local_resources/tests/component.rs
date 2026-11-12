//! A native component ends with its capability.
use super::Pump;
use crate::{
    local_resources::Capability,
    native_renders::{ComponentEndpoint, ComponentRequest},
    tests::ended_errors,
};
use maestro_extensions::Component;
use std::rc::Rc;

/// A component that renders no lines.
struct Empty;

impl Component for Empty {
    fn render(&self, _width: usize) -> Vec<String> {
        Vec::new()
    }
}

#[test]
fn released_native_component_rejects_every_kind_of_operation_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let lease = pump
        .registry()
        .insert(Capability::NativeComponent(Rc::new(Empty)))
        .unwrap();
    let component = ComponentEndpoint(lease.clone());
    let errors = ended_errors(lease, pump, admission, move || {
        [
            component
                .call(ComponentRequest::Render(10))
                .result()
                .unwrap_err(),
            component
                .call(ComponentRequest::Input("a".into()))
                .result()
                .unwrap_err(),
            component
                .call(ComponentRequest::Invalidate)
                .result()
                .unwrap_err(),
            component.call(ComponentRequest::Key).result().unwrap_err(),
        ]
    });
    assert_eq!(errors, ["extension native component resource ended"; 4]);
}

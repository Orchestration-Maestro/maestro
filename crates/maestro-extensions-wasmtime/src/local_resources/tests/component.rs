//! A native component ends with its capability.
use super::ended_endpoint;
use crate::{local_resources::Capability, native_renders::ComponentEndpoint};
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
    let errors = ended_endpoint(
        Capability::NativeComponent(Rc::new(Empty)),
        ComponentEndpoint,
        |component| {
            [
                component.render(10).result().unwrap_err(),
                component.handle_input("a".into()).result().unwrap_err(),
                component.invalidate().result().unwrap_err(),
                component.wants_key_release().result().unwrap_err(),
            ]
        },
    );
    assert_eq!(errors, ["extension native component resource ended"; 4]);
}

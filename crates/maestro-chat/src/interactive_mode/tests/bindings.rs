//! Binding the current session's extensions to the retained mode.
use super::{isolated, started};
use std::rc::Rc;

#[test]
fn native_binding_owned_context_keeps_the_mode_and_then_restores_no_ui() {
    isolated(
        "native_binding_owned_context_keeps_the_mode_and_then_restores_no_ui",
        async |directory| {
            let world = started(directory).await;
            let session = world.mode.runtime_host.session();
            let runner = session.extension_runner();
            let context = runner.create_context();
            assert!(context.has_ui().unwrap());
            let ui = context.ui().unwrap();
            let weak = Rc::downgrade(&world.mode);
            drop(world.mode);
            assert!(
                weak.upgrade().is_some(),
                "an obtained UI keeps the mode alive"
            );
            ui.set_editor_text("retained host".to_owned()).unwrap();
            assert_eq!(ui.get_editor_text().unwrap(), "retained host");
            drop(ui);
            assert!(
                weak.upgrade().is_none(),
                "session and runner do not own the mode"
            );
            assert!(!context.has_ui().unwrap());
            assert!(!runner.has_ui());
            assert_eq!(context.ui().unwrap().get_editor_text().unwrap(), "");
        },
    );
}

//! Open typed message roles through the public browser registry.
#![cfg(test)]
use crate::{message_renderer_support, renderer_recipes};

use maestro_app::AgentMessage;
use maestro_web::{get_message_renderer, register_message_renderer, render_message};
use message_renderer_support::{NativeMessage, Renderer, message};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

/// Missing and replacement lookups preserve open roles and retained callback ownership.
#[test]
fn message_registry_keeps_open_roles_replacements_and_retained_handles() {
    assert!(get_message_renderer::<NativeMessage>("missing-role").is_none());
    assert!(
        render_message(&message("missing-role", Arc::default()))
            .unwrap()
            .is_none()
    );
    let seen = Rc::new(RefCell::new(Vec::new()));
    for role in ["", "ordinary", "日本語", "__proto__"] {
        let input = message(role, Arc::new(AtomicU32::new(7)));
        register_message_renderer(role, recording_renderer(&seen, 1));
        let retained = get_message_renderer::<NativeMessage>(role).unwrap();
        register_message_renderer(role, recording_renderer(&seen, 9));
        retained.render(&input).unwrap();
        render_message(&input).unwrap().unwrap();
        register_message_renderer(role, recording_renderer(&seen, 2));
        render_message(&input).unwrap().unwrap();
    }
    assert_eq!(*seen.borrow(), [1, 9, 2].repeat(4));
    assert!(get_message_renderer::<maestro_app::NoCustomMessages>("ordinary").is_none());
}

/// A callback records which retained or replacement registration was invoked.
fn recording_renderer(
    seen: &Rc<RefCell<Vec<u32>>>,
    version: u32,
) -> Rc<dyn maestro_web::MessageRenderer<NativeMessage>> {
    let seen = seen.clone();
    Rc::new(Renderer::<NativeMessage>(Rc::new(move |_| {
        seen.borrow_mut().push(version);
        Ok(Box::new(renderer_recipes::Caption("owned".into())))
    })))
}

/// Callbacks and replaced user drops run without registry, typed-state or message guards.
#[test]
fn message_callbacks_keep_native_fields_and_allow_reentrant_registration() {
    let handle = Arc::new(AtomicU32::new(1));
    let input = message("native-reentrant", handle.clone());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let captured = seen.clone();
    let original = Arc::clone(&handle);
    register_message_renderer(
        "native-reentrant",
        Rc::new(Renderer::<NativeMessage>(Rc::new(move |message| {
            let AgentMessage::Custom(value) = message else {
                unreachable!()
            };
            let value = value.write().unwrap();
            assert!(Arc::ptr_eq(&value.handle, &original));
            captured
                .borrow_mut()
                .push(value.handle.load(Ordering::Relaxed));
            let seen = captured.clone();
            register_message_renderer(
                "native-reentrant",
                Rc::new(Renderer::<NativeMessage>(Rc::new(move |_| {
                    seen.borrow_mut().push(99);
                    Ok(Box::new(renderer_recipes::Caption("next".into())))
                }))),
            );
            assert!(get_message_renderer::<NativeMessage>("native-reentrant").is_some());
            Ok(Box::new(renderer_recipes::Caption("current".into())))
        }))),
    );
    handle.store(42, Ordering::Relaxed);
    render_message(&input).unwrap().unwrap();
    render_message(&input).unwrap().unwrap();
    assert_eq!(*seen.borrow(), [42, 99]);
    assert_reentrant_drop();
}
/// A user-owned renderer destructor can access the same typed registration table.
fn assert_reentrant_drop() {
    /// Access the registry when the replaced callback releases its final capture.
    struct Reenter(Rc<Cell<bool>>);
    impl Drop for Reenter {
        fn drop(&mut self) {
            self.0
                .set(get_message_renderer::<NativeMessage>("drop-reentrant").is_some());
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let owner = Reenter(dropped.clone());
    register_message_renderer(
        "drop-reentrant",
        Rc::new(Renderer::<NativeMessage>(Rc::new(move |_| {
            let _keep = &owner;
            Ok(Box::new(renderer_recipes::Caption("old".into())))
        }))),
    );
    register_message_renderer(
        "drop-reentrant",
        Rc::new(Renderer::<NativeMessage>(Rc::new(|_| {
            Ok(Box::new(renderer_recipes::Caption("new".into())))
        }))),
    );
    assert!(dropped.get());
}

//! Keyboard and overlay-creation hooks scoped to one overlay transaction.
use crate::browser_fixture_support::{Patch, construct, dom_exception, error, global, object};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsValue;

/// Dispatch one `keydown` of `key` to the page window.
pub(super) fn press_key(key: &str) {
    let event = construct(
        "KeyboardEvent",
        &[&"keydown".into(), &object(&[("key", &key.into())])],
    );
    web_sys::window()
        .unwrap()
        .dispatch_event(&event.into())
        .unwrap();
}

/// Window `keydown` callbacks registered while observed; removal must match registration.
pub(super) struct Keys {
    /// Callbacks registered and not yet removed.
    live: Rc<RefCell<Vec<JsValue>>>,
    /// The replaced registration and removal, restored when finished.
    patches: [Patch; 2],
}

impl Keys {
    /// Observe window `keydown` registration and removal.
    pub(super) fn begin() -> Self {
        let window = global("window");
        let live: Rc<RefCell<Vec<JsValue>>> = Rc::default();
        let added = Rc::clone(&live);
        let add = Patch::method(&window, "addEventListener", move |original, this, args| {
            if args.get(0) == "keydown" {
                added.borrow_mut().push(args.get(1));
            }
            original.apply(&this, &args)
        });
        let removed = Rc::clone(&live);
        let remove = Patch::method(
            &window,
            "removeEventListener",
            move |original, this, args| {
                let callback = args.get(1);
                let position = removed
                    .borrow()
                    .iter()
                    .position(|live| js_sys::Object::is(live, &callback));
                match position {
                    Some(position) if args.get(0) == "keydown" => {
                        removed.borrow_mut().remove(position);
                    }
                    None if args.get(0) == "keydown" => {
                        return Err(error("wrong key callback removed"));
                    }
                    _ => {}
                }
                original.apply(&this, &args)
            },
        );
        Self {
            live,
            patches: [add, remove],
        }
    }

    /// Restore the window and require every callback to be removed.
    pub(super) fn finish(self) {
        let retained = self.live.borrow().len();
        drop(self.patches);
        assert_eq!(retained, 0, "key callback retained after close");
    }
}

/// Refuse creating overlay elements until dropped.
pub(super) fn refuse_creation() -> Patch {
    Patch::method(
        &global("document"),
        "createElement",
        |original, this, args| {
            if args.get(0) == "attachment-overlay" {
                return Err(dom_exception(
                    "controlled overlay creation",
                    "InvalidStateError",
                ));
            }
            original.apply(&this, &args)
        },
    )
}

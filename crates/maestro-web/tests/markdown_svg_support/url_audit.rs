//! Object URL effects recorded where the document observations read them.
use crate::browser_fixture_support::{Patch, get, global, object, set};
use wasm_bindgen::JsValue;

/// Record each object URL the page creates and revokes until dropped: `urlEvents` lists the
/// effects in order, `urlBlobs` maps live URLs to their Blobs and `urlIds` numbers each URL.
#[must_use]
pub(crate) fn install() -> [Patch; 2] {
    let window = global("window");
    let (events, blobs, ids) = (js_sys::Array::new(), js_sys::Map::new(), js_sys::Map::new());
    set(&window, "urlEvents", &events);
    set(&window, "urlBlobs", &blobs);
    set(&window, "urlIds", &ids);
    let url = global("URL");
    let (created, kept, numbered) = (events.clone(), blobs.clone(), ids.clone());
    let create = Patch::method(&url, "createObjectURL", move |original, this, args| {
        let blob = args.get(0);
        let address = original.apply(&this, &args)?;
        let id = JsValue::from(numbered.size() + 1);
        numbered.set(&address, &id);
        kept.set(&address, &blob);
        let effect = object(&[
            ("op", &"create".into()),
            ("id", &id),
            ("type", &get(&blob, "type")),
            ("size", &get(&blob, "size")),
        ]);
        created.push(&effect);
        Ok(address)
    });
    let revoke = Patch::method(&url, "revokeObjectURL", move |original, this, args| {
        let address = args.get(0);
        events.push(&object(&[
            ("op", &"revoke".into()),
            ("id", &ids.get(&address)),
        ]));
        blobs.delete(&address);
        original.apply(&this, &args)
    });
    [create, revoke]
}

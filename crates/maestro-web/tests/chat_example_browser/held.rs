//! Controlled response completion and title-write settlement for the chat example.
use crate::browser_fixture_support::{
    Patch, construct, dom_exception, get, global, invoke, object, signal,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};

/// A streamed reply held after its opening events until the test finishes it.
pub(super) struct HeldResponse {
    /// Settles once the page has received the opening events.
    ready: js_sys::Promise,
    /// Whether the page aborted the request.
    aborted: Rc<Cell<bool>>,
    /// The reply stream's controller, once the page has fetched.
    controller: Rc<RefCell<Option<JsValue>>>,
    /// The reply bytes held back.
    rest: js_sys::Uint8Array,
    /// The replaced page fetch, restored on drop.
    _fetch: Patch,
}

impl HeldResponse {
    /// Answer the next fetch with `body` up to its first block end, holding the rest.
    pub(super) fn install(body: &str) -> Self {
        let split = body.find("event: content_block_stop").unwrap();
        let first = js_sys::Uint8Array::from(&body.as_bytes()[..split]);
        let (ready, started) = signal();
        let aborted = Rc::new(Cell::new(false));
        let controller: Rc<RefCell<Option<JsValue>>> = Rc::default();
        let (seen, held) = (Rc::clone(&aborted), Rc::clone(&controller));
        let fetch = Patch::method(&global("window"), "fetch", move |_, _, args| {
            let request = construct("Request", &[&args.get(0), &args.get(1)]);
            let (seen, aborting) = (Rc::clone(&seen), Rc::clone(&held));
            let on_abort = Closure::once_into_js(move || {
                seen.set(true);
                let error = dom_exception("Aborted", "AbortError");
                invoke(aborting.borrow().as_ref().unwrap(), "error", &[&error]).unwrap();
            });
            let once = object(&[("once", &JsValue::TRUE)]);
            invoke(
                &get(&request, "signal"),
                "addEventListener",
                &[&"abort".into(), &on_abort, &once],
            )?;
            let (first, started, starting) = (first.clone(), started.clone(), Rc::clone(&held));
            let start = Closure::once_into_js(move |value: JsValue| {
                invoke(&value, "enqueue", &[&first]).unwrap();
                *starting.borrow_mut() = Some(value);
                started.call0(&JsValue::NULL).unwrap();
            });
            let stream = construct("ReadableStream", &[&object(&[("start", &start)])]);
            let headers = object(&[("content-type", &"text/event-stream".into())]);
            let response = construct("Response", &[&stream, &object(&[("headers", &headers)])]);
            let url = object(&[("value", &get(&request, "url"))]);
            js_sys::Reflect::define_property(
                response.unchecked_ref::<js_sys::Object>(),
                &"url".into(),
                &url,
            )?;
            Ok(js_sys::Promise::resolve(&response).into())
        });
        Self {
            ready,
            aborted,
            controller,
            rest: js_sys::Uint8Array::from(&body.as_bytes()[split..]),
            _fetch: fetch,
        }
    }

    /// Wait until the page holds the opening events.
    pub(super) async fn ready(&self) {
        crate::contributions_support::settle(self.ready.clone().into())
            .await
            .unwrap();
    }

    /// Whether the page aborted the held request.
    pub(super) fn aborted(&self) -> bool {
        self.aborted.get()
    }

    /// Deliver the held events and end the reply.
    pub(super) fn finish(&self) {
        let controller = self.controller.borrow();
        let controller = controller.as_ref().unwrap();
        invoke(controller, "enqueue", &[&self.rest]).unwrap();
        invoke(controller, "close", &[]).unwrap();
    }
}

/// The last store of one session title, held after the browser keeps it.
pub(super) struct HeldTitleWrite {
    /// Settles once the browser has stored the title.
    ready: js_sys::Promise,
    /// The store request whose success the page has not yet seen.
    request: Rc<RefCell<Option<web_sys::IdbRequest>>>,
    /// The replaced store write, restored on drop.
    _put: Patch,
}

impl HeldTitleWrite {
    /// Hold the success of the next session store whose title is `title`.
    pub(super) fn install(title: &str) -> Self {
        let (ready, entered) = signal();
        let request: Rc<RefCell<Option<web_sys::IdbRequest>>> = Rc::default();
        let (title, held, done) = (title.to_owned(), Rc::clone(&request), Cell::new(false));
        let prototype = get(&global("IDBObjectStore"), "prototype");
        let put = Patch::method(&prototype, "put", move |original, store, args| {
            let stored = original.apply(&store, &args)?;
            let matches = get(&store, "name").as_string().as_deref() == Some("sessions")
                && get(&args.get(0), "title").as_string().as_deref() == Some(title.as_str());
            if done.get() || !matches {
                return Ok(stored);
            }
            // Later writes pass through, as if the original store method were back.
            done.set(true);
            let (entered, kept, request) = (entered.clone(), Rc::clone(&held), stored.clone());
            let on_success = Closure::once_into_js(move |event: web_sys::Event| {
                event.stop_immediate_propagation();
                *kept.borrow_mut() = Some(request.unchecked_into());
                entered.call0(&JsValue::NULL).unwrap();
            });
            let once = object(&[("once", &JsValue::TRUE)]);
            invoke(
                &stored,
                "addEventListener",
                &[&"success".into(), &on_success, &once],
            )?;
            Ok(stored)
        });
        Self {
            ready,
            request,
            _put: put,
        }
    }

    /// Wait until the browser has stored the title.
    pub(super) async fn ready(&self) {
        crate::contributions_support::settle(self.ready.clone().into())
            .await
            .unwrap();
    }

    /// Let the page see the held success.
    pub(super) fn finish(&self) {
        let request = self.request.borrow();
        let success = web_sys::Event::new("success").unwrap();
        request.as_ref().unwrap().dispatch_event(&success).unwrap();
    }
}

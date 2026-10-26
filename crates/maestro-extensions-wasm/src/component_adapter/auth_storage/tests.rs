//! Authored callbacks run through the same bodies as the canonical exports.
use super::*;
use crate::component_adapter::callbacks::{Identity, release};
use crate::tests::controlled::Controlled;
use crate::{AsyncLockUpdate, AuthStorageBackend};
use serde_json::json;
use std::cell::{Cell, RefCell};

/// Raw backend with separately injected acquisition and write failures.
struct Backend {
    /// Text only changes after a successful replacement.
    raw: RefCell<Option<String>>,
    /// The phase that fails, or an empty string for success.
    failure: &'static str,
    /// Counts updates admitted after acquisition.
    calls: Cell<usize>,
}
impl Backend {
    /// Acquisition failure prevents entry to the borrowed native update.
    fn current(&self) -> ExtensionResult<Option<String>> {
        if self.failure == "acquisition failed" {
            return Err(self.failure.into());
        }
        self.calls.set(self.calls.get() + 1);
        Ok(self.raw.borrow().clone())
    }
    /// Missing replacement leaves storage alone; an empty string is still written.
    fn replace(&self, next: Option<String>) -> ExtensionResult<()> {
        if let Some(next) = next {
            if self.failure == "write failed" {
                return Err(self.failure.into());
            }
            self.raw.replace(Some(next));
        }
        Ok(())
    }
}
impl AuthStorageBackend for Backend {
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> ExtensionResult<Option<String>>,
    ) -> ExtensionResult<()> {
        self.replace(update(self.current()?.as_deref())?)
    }
    fn with_lock_async<'a>(&'a self, update: AsyncLockUpdate<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move { self.replace(update(self.current()?).await?) })
    }
}
#[test]
fn credential_backend_callbacks_keep_lock_update_outcomes() {
    crate::tests::block_on(async {
        for (next, update_error, backend_error, expected) in [
            (None, None, "", Ok("current raw text")),
            (Some(""), None, "", Ok("")),
            (Some("replacement"), None, "", Ok("replacement")),
            (
                Some("replacement"),
                Some("update failed"),
                "",
                Err("update failed"),
            ),
            (
                Some("replacement"),
                None,
                "acquisition failed",
                Err("acquisition failed"),
            ),
            (
                Some("replacement"),
                None,
                "write failed",
                Err("write failed"),
            ),
        ] {
            for asynchronous in [false, true] {
                backend_outcome(next, update_error, backend_error, expected, asynchronous).await?;
            }
        }
        Ok::<_, String>(())
    })
    .unwrap();
}
/// Runs the same synchronous or awaited callback body against one supplied update outcome.
async fn backend_outcome(
    next: Option<&str>,
    update_error: Option<&str>,
    backend_error: &'static str,
    expected: Result<&str, &str>,
    asynchronous: bool,
) -> ExtensionResult<()> {
    let host = Controlled::default();
    host.observe().readers.control(&json!({"bind":{"replies":[
        {"method":"create_auth_storage","args":[null],"value":null},
        {"method":"lock_update","args":["current raw text"],"value":next,"error":update_error},
        {"method":"lock_update_async","args":["current raw text"],"value":next,"error":update_error}
    ]}}).to_string());
    let update = host.create_auth_storage(None)?;
    let identity = Identity::new(&host);
    let id = host.callback_id(&identity.handle);
    let backend = Rc::new(Backend {
        raw: RefCell::new(Some("current raw text".into())),
        failure: backend_error,
        calls: Cell::new(0),
    });
    let callable: Rc<dyn AuthStorageBackend> = backend.clone();
    identity.stage(callable);
    let result = if asynchronous {
        invoke_backend_async(&host, id, &update).await
    } else {
        invoke_backend(&host, id, &update)
    };
    assert_eq!(result, expected.map(|_| ()).map_err(str::to_owned));
    assert_eq!(
        backend.raw.borrow().as_deref(),
        Some(expected.unwrap_or("current raw text"))
    );
    let expected_calls = usize::from(backend_error != "acquisition failed");
    assert_eq!(backend.calls.get(), expected_calls);
    assert_eq!(
        host.observe().readers.acquire()?.queries().len(),
        1 + expected_calls
    );
    identity.settle(true);
    release(id);
    Ok(())
}
/// Destruction reenters a surviving callback, detecting table borrows during rejection.
struct Capture {
    /// Number of callbacks whose final capture has retired.
    drops: Rc<Cell<usize>>,
    /// A separate live callback identity.
    probe: u32,
}
impl Drop for Capture {
    fn drop(&mut self) {
        assert_eq!(
            invoke_fallback(self.probe, "probe"),
            Ok(Some("kept".into()))
        );
        self.drops.set(self.drops.get() + 1);
    }
}

/// Rejected backend construction must not enter either author method.
struct RejectedBackend(Capture);
impl AuthStorageBackend for RejectedBackend {
    fn with_lock(
        &self,
        _: &mut dyn FnMut(Option<&str>) -> ExtensionResult<Option<String>>,
    ) -> ExtensionResult<()> {
        panic!("rejected backend entered: {}", self.0.probe)
    }
    fn with_lock_async<'a>(&'a self, _: AsyncLockUpdate<'a>) -> ExtensionFuture<'a, ()> {
        panic!("rejected async backend entered: {}", self.0.probe)
    }
}

#[test]
fn credential_callback_rejection_releases_captures_and_released_ids_fail() {
    let host = Controlled::default();
    host.observe().readers.control(&json!({"bind":{"replies":[
        {"method":"create_auth_storage","args":[null],"value":null},
        {"method":"auth_storage_from_backend","args":[],"value":null,"error":"literal backend rejection"},
        {"method":"set_fallback_resolver","args":[],"value":null,"error":"literal resolver rejection"},
        {"method":"set_runtime_api_key","args":["valid","kept"],"value":null}
    ]}}).to_string());
    let identity = Identity::new(&host);
    let probe = host.callback_id(&identity.handle);
    let resolver: crate::FallbackResolver = Rc::new(|_| Ok(Some("kept".into())));
    identity.stage(resolver);
    identity.settle(true);
    let drops = Rc::new(Cell::new(0));
    let api = crate::ExtensionAPI::new(Rc::new(super::super::host_api::Host(host)));
    let backend = Rc::new(RejectedBackend(Capture {
        drops: drops.clone(),
        probe,
    }));
    assert_eq!(
        AuthStorage::from_storage(&api, backend).err(),
        Some("literal backend rejection".into())
    );
    assert_eq!(drops.get(), 1);
    let owner = AuthStorage::create(&api, None).unwrap();
    let capture = Capture {
        drops: drops.clone(),
        probe,
    };
    assert_eq!(
        owner.set_fallback_resolver(Rc::new(move |_| {
            let _capture = &capture;
            Ok(None)
        })),
        Err("literal resolver rejection".into())
    );
    assert_eq!(drops.get(), 2);
    owner.set_runtime_api_key("valid", "kept").unwrap();
    assert_eq!(invoke_fallback(probe, "probe"), Ok(Some("kept".into())));
    release(probe);
    assert_eq!(
        invoke_fallback(probe, "probe"),
        Err(format!("no callback registered for identity {probe}"))
    );
    assert_eq!(drops.get(), 2);
}

#[test]
fn credential_fallback_preserves_empty_and_supplied_failure() {
    let host = Controlled::default();
    for expected in [
        Ok(None),
        Ok(Some(String::new())),
        Err("literal fallback failure".to_owned()),
    ] {
        let identity = Identity::new(&host);
        let id = host.callback_id(&identity.handle);
        let answer = expected.clone();
        let resolver: crate::FallbackResolver = Rc::new(move |provider| {
            assert_eq!(provider, " literal ");
            answer.clone()
        });
        identity.stage(resolver);
        assert_eq!(invoke_fallback(id, " literal "), expected);
        identity.settle(true);
        release(id);
    }
}

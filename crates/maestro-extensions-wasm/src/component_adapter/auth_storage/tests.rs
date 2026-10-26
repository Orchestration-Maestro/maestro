//! Authored callbacks run through the same bodies as the canonical exports.
use super::*;
use crate::component_adapter::callbacks::{Identity, release};
use crate::tests::controlled::Controlled;
use crate::{AsyncLockUpdate, AuthStorageBackend};
use serde_json::json;
use std::cell::Cell;

/// Backend that checks the actual update result, not a unit-only success.
struct Backend(Rc<Cell<bool>>);
impl AuthStorageBackend for Backend {
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> ExtensionResult<Option<String>>,
    ) -> ExtensionResult<()> {
        assert_eq!(update(Some("current raw text"))?, None);
        Ok(())
    }
    fn with_lock_async<'a>(&'a self, update: AsyncLockUpdate<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            assert_eq!(
                update(Some("current raw text".into())).await?,
                Some(String::new())
            );
            self.0.set(true);
            Ok(())
        })
    }
}
#[test]
fn credential_backend_callbacks_keep_lock_update_outcomes() {
    crate::tests::block_on(async {
        let host = Controlled::default();
        host.observe().readers.control(
            &json!({"bind":{"replies":[
                {"method":"create_auth_storage","args":[null],"value":null},
                {"method":"lock_update","args":["current raw text"],"value":null},
                {"method":"lock_update_async","args":["current raw text"],"value":""}
            ]}})
            .to_string(),
        );
        let update = host.create_auth_storage(None)?;
        let identity = Identity::new(&host);
        let id = host.callback_id(&identity.handle);
        let completed = Rc::new(Cell::new(false));
        let backend: Rc<dyn AuthStorageBackend> = Rc::new(Backend(completed.clone()));
        identity.stage(backend);
        invoke_backend(&host, id, &update)?;
        invoke_backend_async(&host, id, &update).await?;
        assert!(completed.get());
        identity.settle(true);
        release(id);
        Ok::<_, String>(())
    })
    .unwrap();
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

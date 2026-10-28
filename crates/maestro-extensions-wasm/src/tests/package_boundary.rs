//! Fixed-schema package results are checked before reaching author code.
use super::controlled::Controlled;
use crate::component_adapter::Exports;
use crate::package_manager::OperationScope;
use crate::{DefaultPackageManager, Extension, ExtensionAPI, ExtensionFuture};
use serde_json::json;

/// A factory asking for malformed package and pin observations.
struct MalformedPackages;
impl Extension for MalformedPackages {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        Box::pin(async move {
            let packages = DefaultPackageManager::new(api);
            assert!(packages.resolve().await.is_err());
            assert!(
                packages
                    .resolve_extension_sources(vec!["./empty".into()], OperationScope::Project)
                    .await
                    .is_err()
            );
            assert!(
                packages
                    .verify_pin("https://example.test/repo@pin", OperationScope::User)
                    .await
                    .is_err()
            );
            Ok(())
        })
    }
}

#[test]
fn malformed_package_results_are_rejected_at_the_guest_boundary() {
    for malformed in [json!(null), json!({"source": 17})] {
        let host = Controlled::default();
        host.observe().host_operation_result = Some(Ok(malformed));
        let exports = Exports::new(host);
        super::block_on(exports.start::<MalformedPackages>()).unwrap();
    }
}

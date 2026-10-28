//! Factory-time observations through the author settings and package facades.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    DefaultPackageManager, ExtensionAPI, ExtensionResult, package_manager::OperationScope,
    settings_manager::SettingsManager,
};
use serde_json::{Value, json};

/// Reports raw settings or package results before any command context or runner exists.
pub async fn observe(api: &ExtensionAPI) -> ExtensionResult<()> {
    let request = std::env::var("MAESTRO_HOST_OPERATIONS_REQUEST");
    let observed = if request.as_deref() == Ok("packages") {
        packages(api).await
    } else {
        let settings = SettingsManager::new(api.clone());
        let raw = json!([
            settings.get_global_settings().await?,
            settings.get_project_settings().await?
        ]);
        if request.as_deref() == Ok("sdk") {
            json!({"settings": raw, "packages": DefaultPackageManager::new(api.clone()).resolve().await?})
        } else {
            raw
        }
    };
    api.events().emit("host-operations", observed).await
}

/// Calls every package operation in order, keeping native failures as values.
async fn packages(api: &ExtensionAPI) -> Value {
    let packages = DefaultPackageManager::new(api.clone());
    let source = "https://example.test/team/repo@pin";
    let user = OperationScope::User;
    let project = OperationScope::Project;
    let results = [
        packages.resolve().await,
        packages
            .install(source, project)
            .await
            .map(|()| Value::Null),
        packages
            .resolve_extension_sources(vec![source.into()], project)
            .await,
        packages.verify_pin(source, user).await,
        packages
            .restore_pin(source, user)
            .await
            .map(|()| Value::Null),
        packages.verify_pin(source, user).await,
        packages.verify_pin("./developer", user).await,
    ];
    results
        .into_iter()
        .map(|result| match result {
            Ok(value) => json!({ "ok": value }),
            Err(error) => json!({ "err": error }),
        })
        .collect()
}

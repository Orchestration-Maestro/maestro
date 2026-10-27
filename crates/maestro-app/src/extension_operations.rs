//! Settings and package composition for factory-time extension calls.
use maestro_extensions::host_operations::{HostOperation, OperationScope};
use maestro_packages::{DefaultPackageManager, NativePackageOperations, PackageManager};
use maestro_settings::SettingsManager;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

/// The loader's original owners, retained by its extension operations.
pub struct ExtensionOperations {
    /// Raw settings remain live across reloads.
    settings: Rc<RefCell<SettingsManager>>,
    /// The loader's existing resolver and acquisition cache.
    packages: Rc<DefaultPackageManager<NativePackageOperations>>,
}
impl ExtensionOperations {
    /// Retains the existing settings owner rather than opening another manager.
    pub(crate) fn new(
        settings: Rc<RefCell<SettingsManager>>,
        packages: Rc<DefaultPackageManager<NativePackageOperations>>,
    ) -> Rc<Self> {
        Rc::new(Self { settings, packages })
    }

    /// Starts an operation without borrowing settings across an await.
    ///
    /// # Errors
    /// Returns a settings borrow or native operation failure.
    #[must_use]
    pub fn call(self: &Rc<Self>, operation: HostOperation) -> crate::PackageFuture<'static, Value> {
        let owners = self.clone();
        Box::pin(async move {
            match operation {
                HostOperation::ReadSettings(scope) => {
                    let settings = owners
                        .settings
                        .try_borrow()
                        .map_err(std::io::Error::other)?;
                    let raw = match scope {
                        OperationScope::User => settings.get_global_settings(),
                        OperationScope::Project => settings.get_project_settings(),
                    };
                    Ok(Value::Object(raw.0))
                }
                HostOperation::ResolvePackages => owners.resolve_packages().await,
                _ => Err(std::io::Error::other(
                    "Extension package operations are not configured",
                )),
            }
        })
    }
}

impl ExtensionOperations {
    /// Projects the owner's selected roots independently of resource membership.
    async fn resolve_packages(&self) -> std::io::Result<Value> {
        let resolved = self.packages.resolve(None).await?;
        Ok(Value::Array(
            resolved
                .packages
                .into_iter()
                .map(|package| {
                    serde_json::json!({
                        "source": package.source,
                        "identity": package.identity,
                        "scope": package.scope,
                        "installedPath": package.installed_path,
                    })
                })
                .collect(),
        ))
    }
}

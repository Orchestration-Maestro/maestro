use crate::ThrownValue;
use std::path::{Path, PathBuf};
pub(super) trait Platform {
    fn native(&self) -> Result<bool, ThrownValue>;
    fn env(&self, key: &str) -> Result<Option<String>, ThrownValue>;
    fn home(&self) -> Result<PathBuf, ThrownValue>;
    fn join(&self, home: &Path) -> Result<PathBuf, ThrownValue>;
    fn exists(&self, path: &Path) -> bool;
}
#[cfg(not(target_arch = "wasm32"))]
pub(super) struct Host;
#[cfg(not(target_arch = "wasm32"))]
impl Platform for Host {
    fn native(&self) -> Result<bool, ThrownValue> {
        Ok(true)
    }

    #[allow(deprecated)]
    fn home(&self) -> Result<PathBuf, ThrownValue> {
        std::env::home_dir().ok_or_else(|| {
            crate::records::diagnostics::error("Unable to get home directory".into())
        })
    }
    fn join(&self, home: &Path) -> Result<PathBuf, ThrownValue> {
        Ok(home.join(".config/gcloud/application_default_credentials.json"))
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn env(&self, key: &str) -> Result<Option<String>, ThrownValue> {
        Ok(std::env::var_os(key).map(|v| v.to_string_lossy().into_owned()))
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
pub(super) struct Browser;
#[cfg(target_arch = "wasm32")]
pub(super) use Browser as Host;
#[cfg(any(test, target_arch = "wasm32"))]
impl Platform for Browser {
    fn native(&self) -> Result<bool, ThrownValue> {
        Ok(false)
    }
    fn env(&self, _: &str) -> Result<Option<String>, ThrownValue> {
        Err(missing_process())
    }
    fn exists(&self, _: &Path) -> bool {
        false
    }
    fn home(&self) -> Result<PathBuf, ThrownValue> {
        unreachable!("browser has no native home facility")
    }
    fn join(&self, _: &Path) -> Result<PathBuf, ThrownValue> {
        unreachable!("browser has no native path facility")
    }
}
#[cfg(any(test, target_arch = "wasm32"))]
fn missing_process() -> ThrownValue {
    ThrownValue::Error(Box::new(crate::Error {
        name: "ReferenceError".into(),
        message: "process is not defined".into(),
        stack: None,
        code: None,
    }))
}

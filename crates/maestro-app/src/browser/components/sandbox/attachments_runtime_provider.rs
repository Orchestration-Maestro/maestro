//! Live attachment input projected into independent runtime snapshots.
use super::sandbox_runtime_provider::{RuntimeProviderValue, SandboxRuntimeProvider};
use crate::browser::{
    tools::prompts::ATTACHMENTS_RUNTIME_DESCRIPTION, utils::attachment_utils::SharedAttachment,
};
use crate::utils::lock::read;
use indexmap::IndexMap;
use maestro_models::DiagnosticErrorInfo;
use std::{
    marker::PhantomData,
    sync::{Arc, RwLock},
};

/// Supplies attachment globals without parsing or changing their content.
pub struct AttachmentsRuntimeProvider<V: RuntimeProviderValue = serde_json::Value> {
    /// Caller-retained input, read afresh for each projection.
    attachments: Arc<RwLock<Vec<SharedAttachment>>>,
    /// The receiving host's retained value representation.
    value: PhantomData<V>,
}
impl<V: RuntimeProviderValue> AttachmentsRuntimeProvider<V> {
    /// Retain the supplied live attachment list.
    #[must_use]
    pub fn new(attachments: Arc<RwLock<Vec<SharedAttachment>>>) -> Self {
        Self {
            attachments,
            value: PhantomData,
        }
    }
}
impl<V: RuntimeProviderValue> SandboxRuntimeProvider<V> for AttachmentsRuntimeProvider<V> {
    fn get_data(&self) -> Result<IndexMap<String, V>, DiagnosticErrorInfo> {
        // Freeze the input before invoking the caller-supplied value adapter.
        let attachments = read(&self.attachments).clone();
        let snapshot: Vec<_> = attachments
            .iter()
            .map(|value| read(value).clone())
            .collect();
        Ok(IndexMap::from([(
            "attachments".into(),
            V::attachment_array(&snapshot)?,
        )]))
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(include_str!("attachments_runtime_provider/runtime.js").into())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(ATTACHMENTS_RUNTIME_DESCRIPTION.into())
    }
}

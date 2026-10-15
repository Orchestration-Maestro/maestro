//! Full session histories and their indexed metadata.
use super::super::{
    IndexConfig, SessionData, SessionMetadata, Store, StoreConfig, TransactionMode,
};
use maestro_agent::CustomAgentMessages;
use serde::{Serialize, de::DeserializeOwned};
use std::io;
/// Typed access to full histories and indexed metadata.
pub struct SessionsStore(Store);
impl Default for SessionsStore {
    fn default() -> Self {
        Self(Store::new(schema("sessions")))
    }
}
impl std::ops::Deref for SessionsStore {
    type Target = Store;
    fn deref(&self) -> &Store {
        &self.0
    }
}
impl SessionsStore {
    /// Write history first, then metadata, within one adapter-owned transaction.
    ///
    /// # Errors
    /// Returns binding, serialization or adapter failures.
    pub async fn save<C: CustomAgentMessages + Serialize>(
        &self,
        data: &SessionData<C>,
        metadata: &SessionMetadata,
    ) -> io::Result<()> {
        let backend = self.get_backend()?;
        backend
            .transaction(
                &["sessions".into(), "sessions-metadata".into()],
                TransactionMode::Readwrite,
                Box::new(move |tx| {
                    Box::pin(async move {
                        tx.set("sessions", &data.id, Store::encode(data)?).await?;
                        tx.set("sessions-metadata", &metadata.id, Store::encode(metadata)?)
                            .await?;
                        Ok(serde_json::Value::Null)
                    })
                }),
            )
            .await?;
        Ok(())
    }
    /// Delete full history first, then metadata, in one transaction.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn delete(&self, id: &str) -> io::Result<()> {
        let backend = self.get_backend()?;
        backend
            .transaction(
                &["sessions".into(), "sessions-metadata".into()],
                TransactionMode::Readwrite,
                Box::new(move |tx| {
                    Box::pin(async move {
                        tx.delete("sessions", id).await?;
                        tx.delete("sessions-metadata", id).await?;
                        Ok(serde_json::Value::Null)
                    })
                }),
            )
            .await?;
        Ok(())
    }
    /// Delete a session through the same transactional operation.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn delete_session(&self, id: &str) -> io::Result<()> {
        self.delete(id).await
    }
    /// Edit only the raw title, metadata before history, without a transaction.
    ///
    /// # Errors
    /// Stops on the first binding or adapter failure; earlier writes remain visible.
    pub async fn update_title(&self, id: &str, title: &str) -> io::Result<()> {
        for name in ["sessions-metadata", "sessions"] {
            if let Some(mut value) = self
                .get_backend()?
                .get(name, id)
                .await?
                .filter(maestro_json::is_truthy)
            {
                value["title"] = title.into();
                self.get_backend()?.set(name, id, value).await?;
            }
        }
        Ok(())
    }
    /// Forward quota inspection to the currently selected adapter.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn get_quota_info(&self) -> io::Result<super::super::QuotaInfo> {
        self.get_backend()?.get_quota_info().await
    }
    /// Forward persistence admission to the currently selected adapter.
    ///
    /// # Errors
    /// Returns binding or adapter failures.
    pub async fn request_persistence(&self) -> io::Result<bool> {
        self.get_backend()?.request_persistence().await
    }
    /// Read a full history, with absent or null represented as no record.
    ///
    /// # Errors
    /// Returns binding, decoding or adapter failures.
    pub async fn get<C: CustomAgentMessages + DeserializeOwned>(
        &self,
        id: &str,
    ) -> io::Result<Option<SessionData<C>>> {
        self.0.read(id).await
    }
    /// Read the indexed summary without loading its history.
    ///
    /// # Errors
    /// Returns binding, decoding or adapter failures.
    pub async fn get_metadata(&self, id: &str) -> io::Result<Option<SessionMetadata>> {
        self.0.read_from("sessions-metadata", id).await
    }
    /// Enumerate summaries in the adapter's descending modification-index order.
    ///
    /// # Errors
    /// Returns binding, decoding or adapter failures.
    pub async fn get_all_metadata(&self) -> io::Result<Vec<SessionMetadata>> {
        self.get_backend()?
            .get_all_from_index(
                "sessions-metadata",
                "lastModified",
                super::super::IndexDirection::Desc,
            )
            .await?
            .into_iter()
            .map(Store::decode)
            .collect()
    }
    /// Load a full history through the same read operation.
    ///
    /// # Errors
    /// Returns binding, decoding or adapter failures.
    pub async fn load_session<C: CustomAgentMessages + DeserializeOwned>(
        &self,
        id: &str,
    ) -> io::Result<Option<SessionData<C>>> {
        self.get(id).await
    }
    /// Prepare retained state owners synchronously, then persist without a state guard.
    ///
    /// # Errors
    /// Returns binding, serialization or adapter failures.
    pub async fn save_session<C: CustomAgentMessages + Serialize>(
        &self,
        id: &str,
        state: &std::sync::Arc<std::sync::RwLock<maestro_agent::AgentState<C>>>,
        metadata: Option<&SessionMetadata>,
        title: Option<&str>,
    ) -> io::Result<()> {
        let (data, metadata) = {
            let state = state
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            prepare(id, &state, metadata, title, now)
        };
        self.save(&data, &metadata).await
    }
    /// Select the newest locale-compared modification string, retaining ties in listing order.
    ///
    /// # Errors
    /// Returns listing or native locale initialization failures.
    pub async fn get_latest_session_id(&self) -> io::Result<Option<String>> {
        let mut metadata = self.get_all_metadata().await?;
        if metadata.is_empty() {
            return Ok(None);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let collator = maestro_text::collation::Collator::new(&|key| std::env::var(key).ok())?;
        metadata.sort_by(|a, b| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                collator.compare(&b.last_modified, &a.last_modified)
            }
            #[cfg(target_arch = "wasm32")]
            {
                js_sys::JsString::from(b.last_modified.as_str())
                    .locale_compare(
                        &a.last_modified,
                        &js_sys::Array::new(),
                        &js_sys::Object::new(),
                    )
                    .cmp(&0)
            }
        });
        Ok(metadata.into_iter().next().map(|entry| entry.id))
    }
    /// Schema for the separately indexed metadata collection.
    #[must_use]
    pub fn get_metadata_config() -> StoreConfig {
        schema("sessions-metadata")
    }
}
/// Both collections use the same identity and modification index.
fn schema(name: &str) -> StoreConfig {
    StoreConfig {
        name: name.into(),
        key_path: Some("id".into()),
        auto_increment: None,
        indices: Some(vec![IndexConfig {
            name: "lastModified".into(),
            key_path: "lastModified".into(),
            unique: None,
        }]),
    }
}

/// Prepare retained owners before admission to asynchronous storage.
fn prepare<C: CustomAgentMessages>(
    id: &str,
    state: &maestro_agent::AgentState<C>,
    metadata: Option<&SessionMetadata>,
    title: Option<&str>,
    mut now: impl FnMut() -> String,
) -> (SessionData<C>, SessionMetadata) {
    use num_traits::ToPrimitive;
    let metadata = metadata.cloned().unwrap_or_else(|| SessionMetadata {
        id: id.into(),
        title: title.unwrap_or("").into(),
        created_at: now(),
        last_modified: now(),
        message_count: state
            .messages()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            .to_f64()
            .unwrap_or(0.0),
        usage: maestro_models::Usage::default(),
        thinking_level: state.thinking_level.clone(),
        preview: String::new(),
    });
    let data = SessionData {
        id: id.into(),
        title: title
            .filter(|s| !s.is_empty())
            .unwrap_or(&metadata.title)
            .into(),
        model: state.model.clone(),
        thinking_level: state.thinking_level.clone(),
        messages: state.messages().clone(),
        created_at: metadata.created_at.clone(),
        last_modified: now(),
    };
    (data, metadata)
}
/// UTC clock text with millisecond precision.
fn now() -> String {
    use num_traits::ToPrimitive;
    chrono::DateTime::from_timestamp_millis(maestro_models::timestamp_now().to_i64().unwrap_or(0))
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests;

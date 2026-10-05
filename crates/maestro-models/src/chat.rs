//! Lazy chat adapter behind the registered provider interface.
use crate::*;
use std::{future::Future, pin::Pin, sync::Arc};
/// Configurable chat-completions connection using supplied HTTP transport.
/// Completion drains the same stream; no request work is detached.
pub struct ChatConnection {
    transport: Arc<dyn ChatTransport>,
    dialect: ChatDialect,
}
impl ChatConnection {
    /// Store supplied dependencies without I/O or runtime construction.
    pub fn new(transport: Arc<dyn ChatTransport>, dialect: ChatDialect) -> Self {
        Self { transport, dialect }
    }
}
impl Provider for ChatConnection {
    fn supports(&self, operation: &str) -> bool {
        operation == "chat"
    }
    fn normalize_tool_call_id(&self, id: &str, model: &Model, source: &AssistantMessage) -> String {
        if source.provider == model.identity.provider
            && source.model == model.identity.model
            && source.protocol == model.protocol
        {
            return id.into();
        }
        let candidate = |id: &str| {
            if let Some((prefix, _)) = id.split_once('|') {
                prefix
                    .encode_utf16()
                    .map(|u| {
                        if u < 128 && ((u as u8).is_ascii_alphanumeric() || u == 95 || u == 45) {
                            u
                        } else {
                            95
                        }
                    })
                    .take(40)
                    .collect::<Vec<_>>()
            } else {
                let mut units = id.encode_utf16().collect::<Vec<_>>();
                if self.dialect.truncate_plain_call_ids {
                    units.truncate(40);
                    if units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
                        units.pop();
                    }
                }
                units
            }
        };
        let mut ids = source
            .content
            .iter()
            .filter_map(|c| {
                if let AssistantContent::ToolCall(t) = c {
                    Some(t.id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        ids.push(id.into());
        ids.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        ids.dedup();
        let mut used = std::collections::BTreeSet::new();
        let mut assigned = std::collections::BTreeMap::new();
        let mut deferred = Vec::new();
        for original in ids {
            let c = String::from_utf16_lossy(&candidate(&original));
            if used.insert(c.clone()) {
                assigned.insert(original, c);
            } else {
                deferred.push((original, c));
            }
        }
        for (original, c) in deferred {
            let mut ordinal = 1;
            loop {
                let suffix = format!("_{ordinal}");
                let mut units = c.encode_utf16().take(40 - suffix.len()).collect::<Vec<_>>();
                if units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
                    units.pop();
                }
                let c = format!("{}{suffix}", String::from_utf16_lossy(&units));
                if used.insert(c.clone()) {
                    assigned.insert(original, c);
                    break;
                }
                ordinal += 1;
            }
        }
        assigned.remove(id).unwrap_or_else(|| id.into())
    }
    fn stream(
        &self,
        model: Model,
        context: Context,
        options: ProviderOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        if model.protocol != "chat-completions" {
            return Err(Failure::UnsupportedOperation);
        }
        let request = crate::chat_request::encode(&model, &context, &options, &self.dialect)?;
        Ok(Box::new(Source {
            transport: self.transport.clone(),
            request: Some(request),
            options,
            body: None,
            sse: crate::chat_sse::Sse::default(),
            decoder: crate::chat_decode::Decoder::new(model.identity.model.clone()),
            finished: false,
        }))
    }
}
struct Source {
    transport: Arc<dyn ChatTransport>,
    request: Option<ChatHttpRequest>,
    options: ProviderOptions,
    body: Option<Box<dyn ChatHttpBody>>,
    sse: crate::chat_sse::Sse,
    decoder: crate::chat_decode::Decoder,
    finished: bool,
}
impl ProviderStream for Source {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async move {
            loop {
                if let Some(u) = self.decoder.queue.pop_front() {
                    return Some(u);
                }
                if self.finished {
                    return None;
                }
                if let Some(r) = self.request.take() {
                    match crate::chat_retry::setup(self.transport.as_ref(), r, &self.options).await
                    {
                        Ok(r) => self.body = Some(r.body),
                        Err(f) => {
                            self.finished = true;
                            return Some(ProviderUpdate::Error { failure: f });
                        }
                    }
                }
                let read = self.body.as_mut().unwrap().next().await;
                let result = match read {
                    Ok(Some(bytes)) => {
                        let mut result = Ok(());
                        'bytes: for byte in bytes {
                            match self.sse.push(&[byte]) {
                                Ok(frames) => {
                                    for data in frames {
                                        match self.decoder.data(&data) {
                                            Ok(true) => {
                                                self.finished = true;
                                                break 'bytes;
                                            }
                                            Ok(false) => {}
                                            Err(f) => {
                                                result = Err(f);
                                                break 'bytes;
                                            }
                                        }
                                    }
                                }
                                Err(f) => {
                                    result = Err(f);
                                    break;
                                }
                            }
                        }
                        result
                    }
                    Ok(None) => {
                        self.finished = true;
                        self.sse.end().and_then(|()| self.decoder.end())
                    }
                    Err(f) => Err(f),
                };
                if let Err(failure) = result {
                    self.finished = true;
                    self.decoder
                        .queue
                        .push_back(ProviderUpdate::Error { failure });
                }
            }
        })
    }
}

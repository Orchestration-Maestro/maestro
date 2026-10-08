//! Generated session, catalog and event bus resources behind the author ports.

use std::rc::Rc;

use serde_json::Value;

use super::callbacks::Identity;
use crate::agent::AgentMessage;
use crate::bindings::maestro::extension::host;
use crate::compaction::CompactionResult;
use crate::event_bus::{EventBus, EventBusPort, EventListener, Subscription, SubscriptionPort};
use crate::messages::CustomMessageInput;
use crate::model_registry::{ModelRegistry, ModelRegistryPort, ResolvedRequestAuth};
use crate::models::Model;
use crate::session_manager::{
    NewSessionOptions, ReadonlySessionManager, SessionContext, SessionEntry, SessionHeader,
    SessionManager, SessionReaderPort, SessionTreeNode, SessionTreeNodePort, SessionWriterPort,
};
use crate::types::{ExtensionFuture, ExtensionResult};

/// Generated tree node behind the facade.
struct GeneratedNode(host::SessionTreeNode);

/// Wraps generated tree nodes.
fn nodes(nodes: Vec<host::SessionTreeNode>) -> Vec<SessionTreeNode> {
    nodes
        .into_iter()
        .map(|node| SessionTreeNode::new(Rc::new(GeneratedNode(node))))
        .collect()
}

impl SessionTreeNodePort for GeneratedNode {
    fn entry(&self) -> ExtensionResult<SessionEntry> {
        self.0.entry()
    }

    fn children(&self) -> ExtensionResult<Vec<SessionTreeNode>> {
        self.0.children().map(nodes)
    }

    fn label(&self) -> ExtensionResult<Option<String>> {
        self.0.label()
    }

    fn label_timestamp(&self) -> ExtensionResult<Option<String>> {
        self.0.label_timestamp()
    }
}

/// Implements the read-only session operations by forwarding to the generated reader found at
/// `$reader` inside the port value.
macro_rules! reader_port {
    ($port:ident, $reader:tt) => {
        impl SessionReaderPort for $port {
            fn get_cwd(&self) -> ExtensionResult<String> {
                self.$reader.get_cwd()
            }

            fn get_session_dir(&self) -> ExtensionResult<String> {
                self.$reader.get_session_dir()
            }

            fn get_session_id(&self) -> ExtensionResult<String> {
                self.$reader.get_session_id()
            }

            fn get_session_file(&self) -> ExtensionResult<Option<String>> {
                self.$reader.get_session_file()
            }

            fn get_leaf_id(&self) -> ExtensionResult<Option<String>> {
                self.$reader.get_leaf_id()
            }

            fn get_leaf_entry(&self) -> ExtensionResult<Option<SessionEntry>> {
                self.$reader.get_leaf_entry()
            }

            fn get_entry(&self, id: &str) -> ExtensionResult<Option<SessionEntry>> {
                self.$reader.get_entry(id)
            }

            fn get_label(&self, id: &str) -> ExtensionResult<Option<String>> {
                self.$reader.get_label(id)
            }

            fn get_branch(&self, from_id: Option<&str>) -> ExtensionResult<Vec<SessionEntry>> {
                self.$reader.get_branch(from_id)
            }

            fn get_header(&self) -> ExtensionResult<Option<SessionHeader>> {
                self.$reader.get_header()
            }

            fn get_entries(&self) -> ExtensionResult<Vec<SessionEntry>> {
                self.$reader.get_entries()
            }

            fn get_tree(&self) -> ExtensionResult<Vec<SessionTreeNode>> {
                self.$reader.get_tree().map(nodes)
            }

            fn get_session_name(&self) -> ExtensionResult<Option<String>> {
                self.$reader.get_session_name()
            }
        }
    };
}

/// Generated read-only session behind the facade.
struct GeneratedReader(host::ReadonlySessionManager);

reader_port!(GeneratedReader, 0);

/// Wraps a generated read-only session.
pub(super) fn reader(reader: host::ReadonlySessionManager) -> ReadonlySessionManager {
    ReadonlySessionManager::new(Rc::new(GeneratedReader(reader)))
}

/// Generated setup session behind the facade; reads go through the session's own reader.
struct GeneratedWriter {
    /// The write capability.
    writer: host::SessionManager,
    /// The read-only view of the same session.
    reader: host::ReadonlySessionManager,
}

reader_port!(GeneratedWriter, reader);

impl SessionWriterPort for GeneratedWriter {
    fn set_session_file(&self, path: &str) -> ExtensionResult<()> {
        self.writer.set_session_file(path)
    }

    fn new_session(&self, options: Option<NewSessionOptions>) -> ExtensionResult<Option<String>> {
        self.writer.new_session(options.as_ref())
    }

    fn is_persisted(&self) -> ExtensionResult<bool> {
        self.writer.is_persisted()
    }

    fn get_children(&self, parent_id: &str) -> ExtensionResult<Vec<SessionEntry>> {
        self.writer.get_children(parent_id)
    }

    fn build_session_context(&self) -> ExtensionResult<SessionContext> {
        self.writer.build_session_context()
    }

    fn append_message(&self, message: AgentMessage) -> ExtensionResult<String> {
        self.writer.append_message(&message)
    }

    fn append_thinking_level_change(&self, level: &str) -> ExtensionResult<String> {
        self.writer.append_thinking_level_change(level)
    }

    fn append_model_change(&self, provider: &str, model_id: &str) -> ExtensionResult<String> {
        self.writer.append_model_change(provider, model_id)
    }

    fn append_compaction(
        &self,
        compaction: CompactionResult,
        from_hook: Option<bool>,
    ) -> ExtensionResult<String> {
        self.writer.append_compaction(&compaction, from_hook)
    }

    fn append_custom_entry(
        &self,
        custom_type: &str,
        data: Option<Value>,
    ) -> ExtensionResult<String> {
        self.writer
            .append_custom_entry(custom_type, data.map(|data| data.to_string()).as_deref())
    }

    fn append_session_info(&self, name: &str) -> ExtensionResult<String> {
        self.writer.append_session_info(name)
    }

    fn append_custom_message_entry(&self, message: CustomMessageInput) -> ExtensionResult<String> {
        self.writer.append_custom_message_entry(&message)
    }

    fn append_label_change(&self, target_id: &str, label: Option<&str>) -> ExtensionResult<String> {
        self.writer.append_label_change(target_id, label)
    }

    fn branch(&self, from_id: &str) -> ExtensionResult<()> {
        self.writer.branch(from_id)
    }

    fn reset_leaf(&self) -> ExtensionResult<()> {
        self.writer.reset_leaf()
    }

    fn branch_with_summary(
        &self,
        from_id: Option<&str>,
        summary: &str,
        details: Option<Value>,
        from_hook: Option<bool>,
    ) -> ExtensionResult<String> {
        let details = details.map(|details| details.to_string());
        self.writer
            .branch_with_summary(from_id, summary, details.as_deref(), from_hook)
    }

    fn create_branched_session(&self, leaf_id: &str) -> ExtensionResult<Option<String>> {
        self.writer.create_branched_session(leaf_id)
    }
}

/// Wraps a generated setup session.
pub(super) fn writer(writer: host::SessionManager) -> SessionManager {
    let reader = writer.reader();
    SessionManager::new(Rc::new(GeneratedWriter { writer, reader }))
}

/// Generated catalog behind the facade.
struct GeneratedRegistry(host::ModelRegistry);

impl ModelRegistryPort for GeneratedRegistry {
    fn get_all(&self) -> ExtensionResult<Vec<Model>> {
        self.0.get_all()
    }

    fn get_available(&self) -> ExtensionResult<Vec<Model>> {
        self.0.get_available()
    }

    fn find(&self, provider: &str, model_id: &str) -> ExtensionResult<Option<Model>> {
        self.0.find(provider, model_id)
    }

    fn get_api_key_and_headers(&self, model: Model) -> ExtensionFuture<'_, ResolvedRequestAuth> {
        Box::pin(self.0.get_api_key_and_headers(model))
    }
}

/// Wraps a generated catalog.
pub(super) fn registry(registry: host::ModelRegistry) -> ModelRegistry {
    ModelRegistry::new(Rc::new(GeneratedRegistry(registry)))
}

/// Generated subscription behind the facade.
struct GeneratedSubscription(host::Subscription);

impl SubscriptionPort for GeneratedSubscription {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        self.0.unsubscribe()
    }
}

/// Generated event bus behind the facade.
struct GeneratedBus(host::EventBus);

impl EventBusPort for GeneratedBus {
    fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()> {
        Box::pin(self.0.emit(channel.to_owned(), data.to_string()))
    }

    fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription> {
        let identity = Identity::new();
        let subscription = self.0.on(channel, &identity.handle)?;
        identity.keep(listener);
        Ok(Subscription::new(Rc::new(GeneratedSubscription(
            subscription,
        ))))
    }
}

/// Wraps a generated event bus.
pub(super) fn bus(bus: host::EventBus) -> EventBus {
    EventBus::new(Rc::new(GeneratedBus(bus)))
}

//! Controlled wire replies shared by the native and component hosts.
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// One selected response with exact argument expectations.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    /// Public query spelling.
    pub method: String,
    /// Literal operands received by the host.
    pub args: Vec<Value>,
    /// Host result in JSON form.
    pub value: Value,
    /// Optional raw selected-record payload.
    #[serde(default)]
    pub raw: Option<String>,
}
/// A flat fixture node; children are indexes, not recursively decoded nodes.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FlatNode {
    /// Captured entry.
    pub entry: Value,
    /// Ordered child indexes.
    pub children: Vec<usize>,
    /// Captured label.
    pub label: Option<String>,
    /// Captured label timestamp.
    pub label_timestamp: Option<String>,
}
/// Supplied tree roots and nodes.
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tree {
    /// Root indexes in host order.
    pub roots: Vec<usize>,
    /// Independent node records.
    pub nodes: Vec<FlatNode>,
}
/// Live host data of one retained reader.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    /// Replies selected by method and literal arguments.
    #[serde(default)]
    pub replies: Vec<Reply>,
    /// Captured tree supplied by this reader.
    #[serde(default)]
    pub tree: Tree,
    /// Failure returned by every query when supplied.
    #[serde(default)]
    pub error: Option<String>,
    /// Query failure captured by newly returned nodes.
    #[serde(default)]
    pub node_error: Option<String>,
}
/// Host state changes synchronously acknowledged by append-entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    /// Bind a new reader without changing previously retained readers.
    bind: Option<Data>,
    /// Change the retained current reader's state.
    update: Option<Data>,
    /// Reject future acquisitions.
    stale: Option<String>,
}
/// One retained host session.
#[derive(Clone, Default)]
pub struct Reader(pub Arc<Mutex<Data>>);
impl Reader {
    /// Look up the actual route and operands, rejecting unintended calls.
    pub fn reply(&self, method: &str, args: &[Value]) -> Result<Reply, String> {
        let data = self.0.lock().unwrap();
        if let Some(error) = &data.error {
            return Err(error.clone());
        }
        data.replies
            .iter()
            .find(|reply| reply.method == method && reply.args == args)
            .cloned()
            .ok_or_else(|| format!("unexpected reader query: {method}"))
    }
    /// Read a string scalar without normalization.
    pub fn string(&self, method: &str) -> Result<String, String> {
        self.optional(method, &[])?
            .ok_or_else(|| "missing required string".into())
    }
    /// Read an optional string, retaining present empty strings.
    pub fn optional(&self, method: &str, args: &[Value]) -> Result<Option<String>, String> {
        let reply = self.reply(method, args)?;
        Ok(reply.value.as_str().map(str::to_owned))
    }
    /// Encode a selected record, or supply deliberately malformed wire text.
    pub fn record(&self, method: &str, args: &[Value]) -> Result<Option<String>, String> {
        let reply = self.reply(method, args)?;
        Ok(reply
            .raw
            .or_else(|| (!reply.value.is_null()).then(|| reply.value.to_string())))
    }
    /// Return captured nodes independently of this reader.
    pub fn tree(&self) -> Result<Vec<Node>, String> {
        let data = self.0.lock().unwrap();
        if let Some(error) = &data.error {
            return Err(error.clone());
        }
        let tree = Arc::new(data.tree.clone());
        Ok(tree
            .roots
            .iter()
            .map(|index| Node {
                tree: Arc::clone(&tree),
                index: *index,
                error: data.node_error.clone(),
            })
            .collect())
    }
}
/// A retained node of a previously supplied tree.
#[derive(Clone)]
pub struct Node {
    /// Complete flat fixture allocation, retained independently of the reader.
    tree: Arc<Tree>,
    /// This node's identity in the allocation.
    index: usize,
    /// Injected query rejection.
    pub error: Option<String>,
}
impl Node {
    /// Selected captured entry.
    pub fn entry(&self) -> Result<String, String> {
        self.check()?;
        Ok(self.tree.nodes[self.index].entry.to_string())
    }
    /// Captured children in supplied order.
    pub fn children(&self) -> Result<Vec<Self>, String> {
        self.check()?;
        Ok(self.tree.nodes[self.index]
            .children
            .iter()
            .map(|index| Self {
                tree: Arc::clone(&self.tree),
                index: *index,
                error: self.error.clone(),
            })
            .collect())
    }
    /// Captured label.
    pub fn label(&self) -> Result<Option<String>, String> {
        self.check()?;
        Ok(self.tree.nodes[self.index].label.clone())
    }
    /// Captured label timestamp.
    pub fn label_timestamp(&self) -> Result<Option<String>, String> {
        self.check()?;
        Ok(self.tree.nodes[self.index].label_timestamp.clone())
    }
    /// Reject injected failures.
    fn check(&self) -> Result<(), String> {
        self.error
            .as_ref()
            .map_or(Ok(()), |error| Err(error.clone()))
    }
}
/// Mutable binding used only when the author explicitly sends a control operation.
#[derive(Default)]
pub struct Bank {
    /// Current acquisition target.
    current: Reader,
    /// Stale acquisition rejection.
    stale: Option<String>,
}
impl Bank {
    /// Apply a host change before acknowledging the guest's entry call.
    pub fn control(&mut self, text: &str) {
        let control: Control = serde_json::from_str(text).unwrap();
        if let Some(data) = control.bind {
            self.current = Reader(Arc::new(Mutex::new(data)));
            self.stale = None;
        }
        if let Some(data) = control.update {
            *self.current.0.lock().unwrap() = data;
        }
        if self.stale.as_ref().is_none_or(String::is_empty)
            && let Some(stale) = control.stale
        {
            self.stale = Some(stale);
        }
    }
    /// Resolve the current reader at this call, unless acquisition is stale.
    pub fn acquire(&self) -> Result<Reader, String> {
        self.stale
            .as_ref()
            .filter(|message| !message.is_empty())
            .map_or_else(|| Ok(self.current.clone()), |message| Err(message.clone()))
    }
}

use super::host::State;
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::Resource;
impl host::HostReadonlySessionManager for State {
    fn get_cwd(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<String, String>> {
        Ok(self.table.get(&this)?.string("getCwd"))
    }
    fn get_session_dir(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<String, String>> {
        Ok(self.table.get(&this)?.string("getSessionDir"))
    }
    fn get_session_id(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<String, String>> {
        Ok(self.table.get(&this)?.string("getSessionId"))
    }
    fn get_session_file(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.optional("getSessionFile", &[]))
    }
    fn get_leaf_id(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.optional("getLeafId", &[]))
    }
    fn get_leaf_entry(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.record("getLeafEntry", &[]))
    }
    fn get_entry(
        &mut self,
        this: Resource<Reader>,
        id: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.record("getEntry", &[json!(id)]))
    }
    fn get_label(
        &mut self,
        this: Resource<Reader>,
        id: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.optional("getLabel", &[json!(id)]))
    }
    fn get_branch(
        &mut self,
        this: Resource<Reader>,
        from_id: Option<String>,
    ) -> wasmtime::Result<Result<String, String>> {
        Ok(self
            .table
            .get(&this)?
            .record("getBranch", &[json!(from_id)])
            .and_then(|v| v.ok_or_else(|| "missing list".into())))
    }
    fn get_header(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.record("getHeader", &[]))
    }
    fn get_entries(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<String, String>> {
        Ok(self
            .table
            .get(&this)?
            .record("getEntries", &[])
            .and_then(|v| v.ok_or_else(|| "missing list".into())))
    }
    fn get_tree(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Vec<Resource<Node>>, String>> {
        let nodes = match self.table.get(&this)?.tree() {
            Ok(nodes) => nodes,
            Err(e) => return Ok(Err(e)),
        };
        Ok(Ok(nodes
            .into_iter()
            .map(|node| self.push(node))
            .collect::<wasmtime::Result<_>>()?))
    }
    fn get_session_name(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.optional("getSessionName", &[]))
    }
    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
impl host::HostSessionTreeNode for State {
    fn entry(&mut self, this: Resource<Node>) -> wasmtime::Result<Result<String, String>> {
        Ok(self.table.get(&this)?.entry())
    }
    fn children(
        &mut self,
        this: Resource<Node>,
    ) -> wasmtime::Result<Result<Vec<Resource<Node>>, String>> {
        let nodes = match self.table.get(&this)?.children() {
            Ok(nodes) => nodes,
            Err(e) => return Ok(Err(e)),
        };
        Ok(Ok(nodes
            .into_iter()
            .map(|node| self.push(node))
            .collect::<wasmtime::Result<_>>()?))
    }
    fn label(&mut self, this: Resource<Node>) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.label())
    }
    fn label_timestamp(
        &mut self,
        this: Resource<Node>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self.table.get(&this)?.label_timestamp())
    }
    fn drop(&mut self, this: Resource<Node>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

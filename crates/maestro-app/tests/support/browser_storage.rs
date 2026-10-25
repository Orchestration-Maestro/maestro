//! Controlled collection adapter at numbered observable effects.
#![cfg(not(target_arch = "wasm32"))]
pub use crate::support::browser_storage_records::{Row, equal, provider_equal};
use futures_util::future::LocalBoxFuture;
use maestro_app::browser::storage::{
    IndexDirection, QuotaInfo, StorageBackend, StorageOperation, StorageTransaction,
    TransactionMode,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    io,
    rc::Rc,
    sync::Arc,
};

/// A distinguishable source error, without logging stored values.
#[derive(Debug)]
pub struct Cause(pub Arc<()>);
impl std::fmt::Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("controlled adapter failure")
    }
}
impl std::error::Error for Cause {}
/// One-shot callbacks at numbered observable effects.
type EffectHooks = VecDeque<(usize, Box<dyn FnOnce()>)>;
/// Supplied adapter with ordered observation and controllable suspension.
#[derive(Default, Clone)]
pub struct Backend {
    /// Whether effects are transaction-scoped.
    transactional: bool,
    /// Supplied index order.
    pub index: Rc<RefCell<Vec<Value>>>,
    /// One-based failure position across the transcript.
    pub fail_at: Rc<RefCell<Option<usize>>>,
    /// Synchronous callbacks at explicit effect boundaries.
    pub hooks: Rc<RefCell<EffectHooks>>,
    /// Reported quota values.
    pub quota: Rc<RefCell<Option<QuotaInfo>>>,
    /// Persistence decision.
    pub granted: Rc<RefCell<bool>>,
    /// Stored collections, whose object maps preserve insertion order.
    pub data: Rc<RefCell<BTreeMap<String, serde_json::Map<String, Value>>>>,
    /// Complete call transcript.
    pub trace: Rc<RefCell<Vec<Value>>>,
    /// Supplied enumeration, independent of stored map order.
    pub keys: Rc<RefCell<Option<Vec<String>>>>,
    /// Failing operation.
    pub fail: Rc<RefCell<Option<String>>>,
    /// A particular failing read.
    pub fail_key: Rc<RefCell<Option<String>>>,
    /// Original adapter cause identity.
    pub cause: Arc<()>,
    /// Next read completion gate.
    pub read_gate: Rc<RefCell<Option<tokio::sync::oneshot::Receiver<Value>>>>,
    /// Next key enumeration completion gate.
    pub keys_gate: Rc<RefCell<Option<tokio::sync::oneshot::Receiver<Vec<String>>>>>,
}
impl Backend {
    /// New isolated supplied adapter.
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
    /// Seed one collection without an observable adapter call.
    pub fn seed(&self, name: &str, values: Value) {
        let Value::Object(values) = values else {
            panic!("seed requires an object")
        };
        self.data.borrow_mut().insert(name.into(), values);
    }
    /// Record a reached adapter effect or return its original cause.
    fn effect(&self, op: &str, args: Vec<Value>) -> io::Result<()> {
        let fail_key = self.fail_key.borrow().clone();
        let op = if self.transactional {
            match op {
                "get" => "tx.get",
                "set" => "tx.set",
                "delete" => "tx.delete",
                other => other,
            }
        } else {
            op
        };
        let fails = self.fail.borrow().as_deref() == Some(op)
            || (op == "get" && fail_key.as_deref().is_some_and(|key| args[1] == key));
        let mut call = vec![json!(op)];
        call.extend(args);
        self.trace.borrow_mut().push(Value::Array(call));
        let position = self.trace.borrow().len();
        let hook = {
            let mut hooks = self.hooks.borrow_mut();
            if hooks.front().is_some_and(|(at, _)| *at == position) {
                hooks.pop_front().map(|(_, hook)| hook)
            } else {
                None
            }
        };
        if let Some(hook) = hook {
            hook();
        }
        if fails || *self.fail_at.borrow() == Some(position) {
            Err(io::Error::other(Cause(self.cause.clone())))
        } else {
            Ok(())
        }
    }
}
impl StorageBackend for Backend {
    fn get<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<Option<Value>>> {
        Box::pin(async move {
            self.effect("get", vec![json!(s), json!(k)])?;
            let gate = self.read_gate.borrow_mut().take();
            let value = if let Some(gate) = gate {
                Some(gate.await.unwrap())
            } else {
                self.data.borrow().get(s).and_then(|m| m.get(k)).cloned()
            };
            Ok(value.filter(|v| !v.is_null()))
        })
    }
    fn set<'a>(&'a self, s: &'a str, k: &'a str, v: Value) -> LocalBoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.effect("set", vec![json!(s), json!(k), v.clone()])?;
            self.data
                .borrow_mut()
                .entry(s.into())
                .or_default()
                .insert(k.into(), v);
            Ok(())
        })
    }
    fn delete<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.effect("delete", vec![json!(s), json!(k)])?;
            self.data
                .borrow_mut()
                .entry(s.into())
                .or_default()
                .shift_remove(k);
            Ok(())
        })
    }
    fn keys<'a>(
        &'a self,
        s: &'a str,
        prefix: Option<&'a str>,
    ) -> LocalBoxFuture<'a, io::Result<Vec<String>>> {
        Box::pin(async move {
            assert!(prefix.is_none());
            self.effect("keys", vec![json!(s)])?;
            let gate = self.keys_gate.borrow_mut().take();
            if let Some(gate) = gate {
                return Ok(gate.await.unwrap());
            }
            Ok(self.keys.borrow().clone().unwrap_or_else(|| {
                self.data
                    .borrow()
                    .get(s)
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default()
            }))
        })
    }
    fn clear<'a>(&'a self, s: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            self.effect("clear", vec![json!(s)])?;
            self.data.borrow_mut().entry(s.into()).or_default().clear();
            Ok(())
        })
    }
    fn has<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<bool>> {
        Box::pin(async move {
            self.effect("has", vec![json!(s), json!(k)])?;
            Ok(self.data.borrow().get(s).is_some_and(|m| m.contains_key(k)))
        })
    }
    fn get_all_from_index<'a>(
        &'a self,
        s: &'a str,
        i: &'a str,
        d: IndexDirection,
    ) -> LocalBoxFuture<'a, io::Result<Vec<Value>>> {
        Box::pin(async move {
            self.effect(
                "getAllFromIndex",
                vec![
                    json!(s),
                    json!(i),
                    json!(if d == IndexDirection::Desc {
                        "desc"
                    } else {
                        "asc"
                    }),
                ],
            )?;
            Ok(self.index.borrow().clone())
        })
    }
    fn transaction<'a>(
        &'a self,
        names: &'a [String],
        mode: TransactionMode,
        operation: StorageOperation<'a>,
    ) -> LocalBoxFuture<'a, io::Result<Value>> {
        Box::pin(async move {
            self.effect(
                "transaction",
                vec![
                    json!(names),
                    json!(if mode == TransactionMode::Readwrite {
                        "readwrite"
                    } else {
                        "readonly"
                    }),
                ],
            )?;
            let staged = Rc::new(RefCell::new(self.data.borrow().clone()));
            let tx = Self {
                data: staged.clone(),
                transactional: true,
                ..self.clone()
            };
            let value = match operation(Rc::new(tx)).await {
                Ok(value) => value,
                Err(error) => {
                    return Err(error);
                }
            };
            self.trace.borrow_mut().push(json!(["commit"]));
            let committed = staged.borrow().clone();
            *self.data.borrow_mut() = committed;
            Ok(value)
        })
    }
    fn get_quota_info(&self) -> LocalBoxFuture<'_, io::Result<QuotaInfo>> {
        Box::pin(async move {
            self.effect("getQuotaInfo", vec![])?;
            Ok(self.quota.borrow().clone().unwrap_or(QuotaInfo {
                usage: 23.0,
                quota: 101.0,
                percent: 22.772_277_227_722_77,
            }))
        })
    }
    fn request_persistence(&self) -> LocalBoxFuture<'_, io::Result<bool>> {
        Box::pin(async move {
            self.effect("requestPersistence", vec![])?;
            Ok(*self.granted.borrow())
        })
    }
}
impl StorageTransaction for Backend {
    fn get<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<Option<Value>>> {
        StorageBackend::get(self, s, k)
    }
    fn set<'a>(&'a self, s: &'a str, k: &'a str, v: Value) -> LocalBoxFuture<'a, io::Result<()>> {
        StorageBackend::set(self, s, k, v)
    }
    fn delete<'a>(&'a self, s: &'a str, k: &'a str) -> LocalBoxFuture<'a, io::Result<()>> {
        StorageBackend::delete(self, s, k)
    }
}

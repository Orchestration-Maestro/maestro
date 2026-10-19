//! Controlled immediately-ready storage backend shared by SDK and browser request witnesses.
use maestro_web::{
    IndexDirection, QuotaInfo, StorageBackend, StorageOperation, StorageTransaction,
    TransactionMode,
};
use serde_json::{Value, json};
use std::{cell::RefCell, future::Future, io, pin::Pin, rc::Rc};
/// Observer invoked at the recorded storage-read boundary.
type ReadObserver = Rc<dyn Fn(&str)>;
/// Borrowed non-Send adapter future.
type Local<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + 'a>>;
/// Immediately-ready contract adapter; no platform algorithm is claimed.
#[derive(Default)]
pub(crate) struct Adapter {
    /// Complete consumed arguments.
    pub(crate) calls: RefCell<Vec<Value>>,
    /// Optional observer at each reached read, after recording the operation.
    pub(crate) on_read: RefCell<Option<ReadObserver>>,
    /// A reached read that reports a controlled adapter failure.
    pub(crate) fail_key: RefCell<Option<String>>,
    /// Shared collection/key records retained through transaction callbacks.
    pub(crate) values: Rc<RefCell<std::collections::BTreeMap<(String, String), Value>>>,
}
impl StorageBackend for Adapter {
    fn get<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, Option<Value>> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["get", s, k]));
            if let Some(observer) = self.on_read.borrow().as_ref() {
                observer(k);
            }
            if self.fail_key.borrow().as_deref() == Some(k) {
                return Err(io::Error::other("controlled adapter read failure"));
            }
            Ok(self.values.borrow().get(&(s.into(), k.into())).cloned())
        })
    }
    fn set<'a>(&'a self, s: &'a str, k: &'a str, v: Value) -> Local<'a, ()> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["set", s, k]));
            self.values.borrow_mut().insert((s.into(), k.into()), v);
            Ok(())
        })
    }
    fn delete<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, ()> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["delete", s, k]));
            self.values.borrow_mut().remove(&(s.into(), k.into()));
            Ok(())
        })
    }
    fn keys<'a>(&'a self, s: &'a str, p: Option<&'a str>) -> Local<'a, Vec<String>> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["keys", s, p]));
            Ok(vec![p.unwrap_or("absent").into()])
        })
    }
    fn get_all_from_index<'a>(
        &'a self,
        s: &'a str,
        i: &'a str,
        d: IndexDirection,
    ) -> Local<'a, Vec<Value>> {
        Box::pin(async move {
            let d = match d {
                IndexDirection::Asc => "asc",
                IndexDirection::Desc => "desc",
            };
            self.calls.borrow_mut().push(json!(["index", s, i, d]));
            Ok(vec![json!(d)])
        })
    }
    fn clear<'a>(&'a self, s: &'a str) -> Local<'a, ()> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["clear", s]));
            self.values.borrow_mut().retain(|(name, _), _| name != s);
            Ok(())
        })
    }
    fn has<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, bool> {
        Box::pin(async move {
            self.calls.borrow_mut().push(json!(["has", s, k]));
            Ok(self.values.borrow().contains_key(&(s.into(), k.into())))
        })
    }
    fn transaction<'a>(
        &'a self,
        s: &'a [String],
        m: TransactionMode,
        op: StorageOperation<'a>,
    ) -> Local<'a, Value> {
        Box::pin(async move {
            let mode = match m {
                TransactionMode::Readonly => "readonly",
                TransactionMode::Readwrite => "readwrite",
            };
            self.calls
                .borrow_mut()
                .push(json!(["transaction", s, mode]));
            op(Rc::new(Self {
                values: self.values.clone(),
                ..Self::default()
            }))
            .await
        })
    }
    fn get_quota_info(&self) -> Local<'_, QuotaInfo> {
        Box::pin(async {
            Ok(QuotaInfo {
                usage: 3.0,
                quota: 12.0,
                percent: 25.0,
            })
        })
    }
    fn request_persistence(&self) -> Local<'_, bool> {
        Box::pin(async { Ok(false) })
    }
}
impl StorageTransaction for Adapter {
    fn get<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, Option<Value>> {
        StorageBackend::get(self, s, k)
    }
    fn set<'a>(&'a self, s: &'a str, k: &'a str, v: Value) -> Local<'a, ()> {
        StorageBackend::set(self, s, k, v)
    }
    fn delete<'a>(&'a self, s: &'a str, k: &'a str) -> Local<'a, ()> {
        StorageBackend::delete(self, s, k)
    }
}

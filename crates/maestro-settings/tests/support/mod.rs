#![allow(dead_code)]
use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use serde_json::{Value, json};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
pub type Job = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type Spawn = Arc<dyn Fn(Job) + Send + Sync>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}
#[derive(Clone, Default)]
pub struct Scheduler(Arc<Mutex<Vec<Job>>>);
impl Scheduler {
    pub fn spawn(&self) -> Spawn {
        let jobs = self.0.clone();
        Arc::new(move |job| jobs.lock().unwrap().push(job))
    }
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }
    pub fn drive(&self) {
        let jobs = std::mem::take(&mut *self.0.lock().unwrap());
        for job in jobs {
            block_on(job);
        }
    }
}
pub fn raw(storage: &dyn SettingsStorage, scope: SettingsScope) -> Option<String> {
    let mut result = None;
    storage
        .with_lock(scope, &mut |text| {
            result = text.map(str::to_owned);
            Ok(None)
        })
        .unwrap();
    result
}
pub fn put(storage: &dyn SettingsStorage, scope: SettingsScope, text: &str) {
    storage
        .with_lock(scope, &mut |_| Ok(Some(text.into())))
        .unwrap();
}
pub fn disk(storage: &dyn SettingsStorage, scope: SettingsScope) -> Value {
    serde_json::from_str(&raw(storage, scope).unwrap()).unwrap()
}
pub fn seeded(
    global: Value,
    project: Value,
) -> (SettingsManager, Arc<InMemorySettingsStorage>, Scheduler) {
    let storage = Arc::new(InMemorySettingsStorage::new());
    put(storage.as_ref(), SettingsScope::Global, &global.to_string());
    put(
        storage.as_ref(),
        SettingsScope::Project,
        &project.to_string(),
    );
    let scheduler = Scheduler::default();
    let manager = SettingsManager::from_storage(storage.clone(), scheduler.spawn());
    (manager, storage, scheduler)
}
pub fn memory(seed: Value) -> (SettingsManager, Scheduler) {
    let scheduler = Scheduler::default();
    (
        SettingsManager::in_memory(seed, scheduler.spawn()).unwrap(),
        scheduler,
    )
}
pub fn empty() -> (SettingsManager, Arc<InMemorySettingsStorage>, Scheduler) {
    seeded(json!({}), json!({}))
}
#[derive(Debug)]
pub struct Sentinel(pub usize);
impl std::fmt::Display for Sentinel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sentinel-{}", self.0)
    }
}
impl std::error::Error for Sentinel {}
#[derive(Default)]
pub struct Controlled {
    pub inner: InMemorySettingsStorage,
    pub writes: Mutex<Vec<(SettingsScope, String)>>,
    pub failures: Mutex<Vec<SettingsScope>>,
}
impl SettingsStorage for Controlled {
    fn with_lock(
        &self,
        scope: SettingsScope,
        operation: &mut dyn FnMut(Option<&str>) -> Result<Option<String>, Error>,
    ) -> Result<(), Error> {
        self.inner.with_lock(scope, &mut |text| {
            let next = operation(text)?;
            if let Some(next) = next.as_ref() {
                self.writes.lock().unwrap().push((scope, next.clone()));
                let mut failures = self.failures.lock().unwrap();
                if failures.first() == Some(&scope) {
                    failures.remove(0);
                    return Err(Box::new(Sentinel(failures.len())));
                }
            }
            Ok(next)
        })
    }
}
pub struct Scratch {
    pub root: std::path::PathBuf,
}
impl Scratch {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let root = std::env::temp_dir().join(format!(
                "maestro-settings-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("{e}"),
            }
        }
    }
    pub fn storage(&self) -> maestro_settings::FileSettingsStorage {
        maestro_settings::FileSettingsStorage::new(
            &self.root.join("cwd"),
            &self.root.join("user"),
            ".maestro",
        )
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

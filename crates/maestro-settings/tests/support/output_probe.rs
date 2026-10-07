use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use std::{future::Future,pin::Pin,sync::{Arc,Mutex},task::{Context,Poll,Wake,Waker}};
fn run<F:Future>(future:F)->F::Output {
    struct Notify;impl Wake for Notify {fn wake(self:Arc<Self>){}}
    let waker=Waker::from(Arc::new(Notify));let mut cx=Context::from_waker(&waker);let mut future=std::pin::pin!(future);
    loop {if let Poll::Ready(v)=future.as_mut().poll(&mut cx){return v;}std::thread::yield_now();}
}
fn main() {
    type Job=Pin<Box<dyn Future<Output=()>+Send>>;
    let jobs=Arc::new(Mutex::new(Vec::<Job>::new()));let queued=jobs.clone();let spawn=Arc::new(move|job:Job|queued.lock().unwrap().push(job));
    let root=std::path::PathBuf::from(std::env::args_os().nth(1).unwrap());
    let manager=SettingsManager::create(&root.join("cwd"),&root.join("user"),".maestro",spawn.clone());
    manager.set_theme("quiet".into());for job in std::mem::take(&mut *jobs.lock().unwrap()){run(job);}run(manager.flush());run(manager.reload());assert!(manager.drain_errors().is_empty());
    let storage=Arc::new(InMemorySettingsStorage::new());storage.with_lock(SettingsScope::Global,&mut |_|Ok(Some("{".into()))).unwrap();
    let manager=SettingsManager::from_storage(storage.clone(),spawn);manager.set_theme("session-only".into());run(manager.flush());run(manager.reload());assert_eq!(manager.drain_errors().len(),2);
    storage.with_lock(SettingsScope::Global,&mut |_|Ok(Some("{}".into()))).unwrap();run(manager.reload());manager.set_theme("repaired".into());for job in std::mem::take(&mut *jobs.lock().unwrap()){run(job);}run(manager.flush());assert_eq!(manager.get_theme().as_deref(),Some("repaired"));assert!(manager.drain_errors().is_empty());
}

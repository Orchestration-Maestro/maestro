use std::{future::Future, pin::Pin, sync::Arc};
#[cfg(not(target_arch = "wasm32"))]
pub(super) type Work = Pin<Box<dyn Future<Output = ()> + Send>>;
#[cfg(target_arch = "wasm32")]
pub(super) type Work = Pin<Box<dyn Future<Output = ()>>>;
#[cfg(not(target_arch = "wasm32"))]
pub(super) trait HostBounds: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync> HostBounds for T {}
#[cfg(target_arch = "wasm32")]
pub(super) trait HostBounds {}
#[cfg(target_arch = "wasm32")]
impl<T> HostBounds for T {}
pub(super) trait Host: HostBounds {
    fn clock(&self) -> f64;
    fn random(&self) -> f64;
    fn spawn(&self, work: Work);
    fn microtask(&self) -> Work;
    fn timer(&self, milliseconds: f64) -> Work;
    fn stderr(&self, text: &str);
}
pub(super) struct Production {
    #[cfg(not(target_arch = "wasm32"))]
    executor: Arc<native::Executor>,
}
pub(super) fn production() -> Arc<dyn Host> {
    Arc::new(Production::new())
}
pub(super) fn clock() -> f64 {
    Production::new().clock()
}
impl Host for Production {
    fn clock(&self) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as f64
        }
        #[cfg(target_arch = "wasm32")]
        {
            js_sys::Date::now()
        }
    }
    fn random(&self) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut bytes = [0; 8];
            getrandom::fill(&mut bytes).unwrap();
            (u64::from_ne_bytes(bytes) >> 11) as f64 / 9007199254740992.0
        }
        #[cfg(target_arch = "wasm32")]
        {
            js_sys::Math::random()
        }
    }
    fn spawn(&self, work: Work) {
        #[cfg(not(target_arch = "wasm32"))]
        self.executor.spawn(work);
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(work);
    }
    fn microtask(&self) -> Work {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Box::pin(native::Microtask(false))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(browser_wait(None))
        }
    }
    fn timer(&self, milliseconds: f64) -> Work {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.executor
                .timer(normalize_native_delay(milliseconds, self))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(browser_wait(Some(milliseconds)))
        }
    }
    fn stderr(&self, text: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            eprint!("{text}");
        }
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let console = js_sys::Reflect::get(&js_sys::global(), &"console".into()).unwrap();
            let error = js_sys::Reflect::get(&console, &"error".into())
                .unwrap()
                .unchecked_into::<js_sys::Function>();
            let _ = error.call1(&console, &text.into());
        }
    }
}
#[cfg(target_arch = "wasm32")]
async fn browser_wait(milliseconds: Option<f64>) {
    use wasm_bindgen::JsCast;
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let global = js_sys::global();
        let name = if milliseconds.is_some() {
            "setTimeout"
        } else {
            "queueMicrotask"
        };
        let function = js_sys::Reflect::get(&global, &name.into())
            .unwrap()
            .unchecked_into::<js_sys::Function>();
        if let Some(delay) = milliseconds {
            function.call2(&global, &resolve, &delay.into()).unwrap();
        } else {
            function.call1(&global, &resolve).unwrap();
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

impl Production {
    fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            executor: Arc::new(native::Executor::default()),
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{
            Mutex, OnceLock,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll, Wake},
        time::{Duration, Instant},
    };
    #[derive(Default)]
    pub(super) struct Executor {
        queue: Mutex<Queue>,
        runtime: OnceLock<tokio::runtime::Handle>,
        timers: Mutex<Timers>,
        timer_changed: tokio::sync::Notify,
    }
    #[derive(Default)]
    struct Queue {
        ready: VecDeque<Arc<Task>>,
        running: bool,
    }
    struct Task {
        executor: Arc<Executor>,
        work: Mutex<Option<Work>>,
        queued: AtomicBool,
    }
    impl Wake for Task {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            if !self.queued.swap(true, Ordering::SeqCst) {
                self.executor.enqueue(self.clone());
            }
        }
    }
    pub(super) struct Microtask(pub bool);
    impl Future for Microtask {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.0 {
                Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
    #[derive(Default)]
    struct Timers {
        entries: Vec<(Instant, u64, tokio::sync::oneshot::Sender<()>)>,
        next: u64,
        running: bool,
    }
    impl Executor {
        fn runtime(&self) -> &tokio::runtime::Handle {
            self.runtime.get_or_init(runtime)
        }
        pub(super) fn spawn(self: &Arc<Self>, work: Work) {
            self.enqueue(Arc::new(Task {
                executor: self.clone(),
                work: Mutex::new(Some(work)),
                queued: AtomicBool::new(true),
            }));
        }
        fn enqueue(self: &Arc<Self>, task: Arc<Task>) {
            let start = {
                let mut q = self.queue.lock().unwrap();
                q.ready.push_back(task);
                if q.running {
                    false
                } else {
                    q.running = true;
                    true
                }
            };
            if start {
                let owner = self.clone();
                self.runtime().spawn(async move {
                    owner.dispatch().await;
                });
            }
        }
        async fn dispatch(self: Arc<Self>) {
            loop {
                let task = {
                    let mut q = self.queue.lock().unwrap();
                    let task = q.ready.pop_front();
                    if task.is_none() {
                        q.running = false;
                    }
                    task
                };
                let Some(task) = task else { return };
                task.queued.store(false, Ordering::SeqCst);
                let mut work = task.work.lock().unwrap().take().unwrap();
                let waker = std::task::Waker::from(task.clone());
                let result = work.as_mut().poll(&mut Context::from_waker(&waker));
                if result.is_pending() {
                    *task.work.lock().unwrap() = Some(work);
                }
            }
        }
        pub(super) fn timer(self: &Arc<Self>, delay: f64) -> Work {
            let (send, receive) = tokio::sync::oneshot::channel();
            let due = Instant::now() + Duration::from_millis(delay as u64);
            let start = {
                let mut timers = self.timers.lock().unwrap();
                let sequence = timers.next;
                timers.next += 1;
                timers.entries.push((due, sequence, send));
                if timers.running {
                    false
                } else {
                    timers.running = true;
                    true
                }
            };
            self.timer_changed.notify_one();
            if start {
                let owner = self.clone();
                self.runtime().spawn(async move {
                    owner.drive_timers().await;
                });
            }
            Box::pin(async move {
                let _ = receive.await;
            })
        }
        async fn drive_timers(self: Arc<Self>) {
            loop {
                let next = {
                    let mut timers = self.timers.lock().unwrap();
                    timers.entries.sort_by_key(|(due, id, _)| (*due, *id));
                    let next = timers.entries.first().map(|t| t.0);
                    if next.is_none() {
                        timers.running = false;
                    }
                    next
                };
                let Some(next) = next else { return };
                let mut sleep = Box::pin(tokio::time::sleep_until(next.into()));
                let mut changed = Box::pin(self.timer_changed.notified());
                std::future::poll_fn(|cx| {
                    if changed.as_mut().poll(cx).is_ready() || sleep.as_mut().poll(cx).is_ready() {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                })
                .await;
                let due = {
                    let mut timers = self.timers.lock().unwrap();
                    timers.entries.sort_by_key(|(due, id, _)| (*due, *id));
                    let count = timers
                        .entries
                        .iter()
                        .take_while(|(due, _, _)| *due <= Instant::now())
                        .count();
                    timers.entries.drain(..count).collect::<Vec<_>>()
                };
                for (_, _, send) in due {
                    let _ = send.send(());
                }
            }
        }
    }
    fn runtime() -> tokio::runtime::Handle {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            return handle;
        }
        static FALLBACK: OnceLock<tokio::runtime::Handle> = OnceLock::new();
        FALLBACK
            .get_or_init(|| {
                let (tx, rx) = std::sync::mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .unwrap();
                    tx.send(runtime.handle().clone()).unwrap();
                    runtime.block_on(std::future::pending::<()>());
                });
                rx.recv().unwrap()
            })
            .clone()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn normalize_native_delay(delay: f64, host: &dyn Host) -> f64 {
    if delay > 2147483647.0 {
        host.stderr(&format!(
            "{} does not fit into a 32-bit signed integer.\nTimeout duration was set to 1.\n",
            ryu_js::Buffer::new().format(delay)
        ));
        1.0
    } else if !delay.is_finite() || delay < 1.0 {
        1.0
    } else {
        delay.trunc()
    }
}

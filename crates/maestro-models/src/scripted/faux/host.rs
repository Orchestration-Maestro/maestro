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
    executor: Arc<executor::Executor>,
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
            signed_milliseconds(std::time::SystemTime::now())
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
        self.executor.spawn(work);
    }
    fn microtask(&self) -> Work {
        Box::pin(executor::Microtask(false))
    }
    fn timer(&self, milliseconds: f64) -> Work {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let delay = normalize_native_delay(milliseconds, self);
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(executor::BrowserTimer::new(milliseconds))
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
impl Production {
    fn new() -> Self {
        Self {
            executor: Arc::new(executor::Executor::default()),
        }
    }
}
mod executor {
    use super::*;
    use std::{
        collections::{HashMap, VecDeque},
        sync::{Mutex, Weak},
        task::{Context, Poll, Wake, Waker},
    };
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    struct Key {
        identity: u64,
        generation: u64,
    }
    struct Task {
        work: Option<Work>,
        queued: bool,
    }
    #[derive(Default)]
    pub(super) struct Executor {
        queue: Mutex<Queue>,
        #[cfg(test)]
        callbacks: Option<Arc<dyn Fn(Step) + Send + Sync>>,
    }
    #[derive(Default)]
    struct Queue {
        tasks: HashMap<Key, Task>,
        ready: VecDeque<Key>,
        free: Vec<u64>,
        next_identity: u64,
        next_generation: u64,
        running: bool,
    }
    struct TaskWake {
        #[cfg(not(target_arch = "wasm32"))]
        executor: Weak<Executor>,
        #[cfg(target_arch = "wasm32")]
        executor: u64,
        key: Key,
    }
    impl Wake for TaskWake {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            #[cfg(not(target_arch = "wasm32"))]
            let owner = self.executor.upgrade();
            #[cfg(target_arch = "wasm32")]
            let owner = BROWSER_EXECUTORS.with(|executors| {
                executors
                    .borrow()
                    .get(&self.executor)
                    .and_then(Weak::upgrade)
            });
            if let Some(owner) = owner {
                owner.wake(self.key);
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
    // A scheduled step owns this guard even before it starts. Cancellation and
    // unwinding release the dispatch flag rather than stranding queued work.
    pub(super) struct Step {
        owner: Arc<Executor>,
        started: bool,
    }
    impl Step {
        pub(super) fn run(mut self) {
            self.started = true;
            self.owner.dispatch();
        }
    }
    impl Drop for Step {
        fn drop(&mut self) {
            let restart = {
                let mut queue = self.owner.queue.lock().unwrap();
                queue.running = !queue.ready.is_empty();
                queue.running
            };
            if restart {
                self.owner.schedule(!self.started);
            }
        }
    }
    impl Executor {
        #[cfg(test)]
        pub(super) fn with_callbacks(callbacks: Arc<dyn Fn(Step) + Send + Sync>) -> Self {
            Self {
                callbacks: Some(callbacks),
                ..Default::default()
            }
        }
        pub(super) fn spawn(self: &Arc<Self>, work: Work) {
            let start = {
                let mut queue = self.queue.lock().unwrap();
                let identity = queue.free.pop().unwrap_or_else(|| {
                    let identity = queue.next_identity;
                    queue.next_identity += 1;
                    identity
                });
                let key = Key {
                    identity,
                    generation: queue.next_generation,
                };
                queue.next_generation += 1;
                queue.tasks.insert(
                    key,
                    Task {
                        work: Some(work),
                        queued: true,
                    },
                );
                queue.ready.push_back(key);
                let start = !queue.running;
                queue.running = true;
                start
            };
            if start {
                self.schedule(false);
            }
        }
        fn wake(self: &Arc<Self>, key: Key) {
            let start = {
                let mut queue = self.queue.lock().unwrap();
                let Some(task) = queue.tasks.get_mut(&key) else {
                    return;
                };
                if task.queued {
                    return;
                }
                task.queued = true;
                queue.ready.push_back(key);
                let start = !queue.running;
                queue.running = true;
                start
            };
            if start {
                self.schedule(false);
            }
        }
        fn schedule(self: &Arc<Self>, fallback: bool) {
            let step = Step {
                owner: self.clone(),
                started: false,
            };
            #[cfg(test)]
            if let Some(callbacks) = &self.callbacks {
                callbacks(step);
                return;
            }
            #[cfg(not(target_arch = "wasm32"))]
            runtime(fallback).spawn(async move {
                step.run();
            });
            #[cfg(target_arch = "wasm32")]
            {
                let _ = fallback;
                browser_callback("queueMicrotask", None, move || step.run());
            }
        }
        fn dispatch(self: &Arc<Self>) {
            let next = {
                let mut queue = self.queue.lock().unwrap();
                queue.ready.pop_front().and_then(|key| {
                    let task = queue.tasks.get_mut(&key)?;
                    task.queued = false;
                    task.work.take().map(|work| (key, work))
                })
            };
            let Some((key, mut work)) = next else { return };
            #[cfg(not(target_arch = "wasm32"))]
            let owner = Arc::downgrade(self);
            #[cfg(target_arch = "wasm32")]
            let owner = browser_executor(self);
            let waker = Waker::from(Arc::new(TaskWake {
                executor: owner,
                key,
            }));
            let result = work.as_mut().poll(&mut Context::from_waker(&waker));
            let removed = {
                let mut queue = self.queue.lock().unwrap();
                if result.is_pending() {
                    queue.tasks.get_mut(&key).unwrap().work = Some(work);
                    None
                } else {
                    queue.free.push(key.identity);
                    queue.tasks.remove(&key)
                }
            };
            // Dropping a future may also invoke caller code.
            drop(removed);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn runtime(fallback: bool) -> tokio::runtime::Handle {
        if !fallback && let Ok(handle) = tokio::runtime::Handle::try_current() {
            return handle;
        }
        static FALLBACK: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();
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
    #[cfg(target_arch = "wasm32")]
    thread_local! {
        static BROWSER_EXECUTORS: std::cell::RefCell<HashMap<u64, Weak<Executor>>> = std::cell::RefCell::default();
        static NEXT_BROWSER_EXECUTOR: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    #[cfg(target_arch = "wasm32")]
    fn browser_executor(owner: &Arc<Executor>) -> u64 {
        BROWSER_EXECUTORS.with(|executors| {
            let mut executors = executors.borrow_mut();
            if let Some((&id, _)) = executors
                .iter()
                .find(|(_, weak)| weak.ptr_eq(&Arc::downgrade(owner)))
            {
                return id;
            }
            executors.retain(|_, weak| weak.strong_count() > 0);
            let id = NEXT_BROWSER_EXECUTOR.with(|next| {
                let id = next.get();
                next.set(id + 1);
                id
            });
            executors.insert(id, Arc::downgrade(owner));
            id
        })
    }
    #[cfg(target_arch = "wasm32")]
    fn browser_callback(name: &str, milliseconds: Option<f64>, callback: impl FnOnce() + 'static) {
        use wasm_bindgen::{JsCast, closure::Closure};
        let callback = Closure::once_into_js(callback);
        let global = js_sys::global();
        let function = js_sys::Reflect::get(&global, &name.into())
            .unwrap()
            .unchecked_into::<js_sys::Function>();
        if let Some(delay) = milliseconds {
            function.call2(&global, &callback, &delay.into()).unwrap();
        } else {
            function.call1(&global, &callback).unwrap();
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) struct BrowserTimer {
        milliseconds: f64,
        ready: std::rc::Rc<std::cell::Cell<bool>>,
        started: bool,
    }
    #[cfg(target_arch = "wasm32")]
    impl BrowserTimer {
        pub(super) fn new(milliseconds: f64) -> Self {
            Self {
                milliseconds,
                ready: Default::default(),
                started: false,
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    impl Future for BrowserTimer {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.ready.get() {
                return Poll::Ready(());
            }
            if !self.started {
                self.started = true;
                let ready = self.ready.clone();
                let waker = cx.waker().clone();
                browser_callback("setTimeout", Some(self.milliseconds), move || {
                    ready.set(true);
                    waker.wake();
                });
            }
            Poll::Pending
        }
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
#[cfg(not(target_arch = "wasm32"))]
fn signed_milliseconds(time: std::time::SystemTime) -> f64 {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as f64,
        Err(error) => -(error.duration().as_millis() as f64),
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
#[test]
fn faux_clock_converts_pre_epoch_milliseconds() {
    use std::time::{Duration, UNIX_EPOCH};
    assert_eq!(
        signed_milliseconds(UNIX_EPOCH - Duration::from_millis(1234)),
        -1234.0
    );
    assert_eq!(signed_milliseconds(UNIX_EPOCH), 0.0);
    assert_eq!(
        signed_milliseconds(UNIX_EPOCH + Duration::from_millis(1234)),
        1234.0
    );
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[test]
fn faux_browser_callback_steps_preserve_microtask_markers() {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::Mutex,
        task::{Context as TaskContext, Poll, Waker},
    };
    type Callback = Box<dyn FnOnce() + Send>;
    let callbacks = Arc::new(Mutex::new(VecDeque::<Callback>::new()));
    let scheduled = callbacks.clone();
    let executor = Arc::new(executor::Executor::with_callbacks(Arc::new(move |step| {
        scheduled
            .lock()
            .unwrap()
            .push_back(Box::new(move || step.run()));
    })));
    let host = Arc::new(Production { executor });
    let r = register_with_host(
        RegisterFauxProviderOptions {
            api: Some("browser-callback-markers".into()),
            ..Default::default()
        },
        host,
    );
    let order = Arc::new(Mutex::new(vec![]));
    let factory_order = order.clone();
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        move |_, _, _, _| {
            factory_order.lock().unwrap().push("factory");
            Box::pin(async {
                Ok(faux_assistant_message(
                    FauxAssistantContent::Text("text".into()),
                    Default::default(),
                ))
            })
        },
    ))]);
    let events = stream(
        r.get_model(None).unwrap(),
        Context {
            system_prompt: None,
            messages: vec![],
            tools: None,
        },
        None,
    )
    .unwrap();
    let marker_order = order.clone();
    let marker_callbacks = callbacks.clone();
    callbacks.lock().unwrap().push_back(Box::new(move || {
        marker_order.lock().unwrap().push("m1");
        marker_callbacks
            .lock()
            .unwrap()
            .push_back(Box::new(move || {
                marker_order.lock().unwrap().push("m2");
            }));
    }));
    loop {
        let callback = callbacks.lock().unwrap().pop_front();
        let Some(callback) = callback else { break };
        callback();
    }
    assert_eq!(*order.lock().unwrap(), ["m1", "factory", "m2"]);
    let mut result = Box::pin(events.result());
    assert!(matches!(
        result
            .as_mut()
            .poll(&mut TaskContext::from_waker(Waker::noop())),
        Poll::Ready(_)
    ));
    r.unregister();
}

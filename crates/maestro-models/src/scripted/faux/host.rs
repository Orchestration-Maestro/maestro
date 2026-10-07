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
pub(super) struct Production;
pub(super) fn production() -> Arc<dyn Host> {
    Arc::new(Production)
}
pub(super) fn clock() -> f64 {
    Production.clock()
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
        #[cfg(not(target_arch = "wasm32"))]
        runtime().spawn(work);
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(work);
    }
    fn microtask(&self) -> Work {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Box::pin(tokio::task::yield_now())
        }
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async {
                let _ = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(
                    &wasm_bindgen::JsValue::UNDEFINED,
                ))
                .await;
            })
        }
    }
    fn timer(&self, milliseconds: f64) -> Work {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let delay = std::time::Duration::from_secs_f64(milliseconds / 1000.0);
            Box::pin(async move {
                tokio::time::sleep(delay).await;
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async move {
                use wasm_bindgen::JsCast;
                let promise = js_sys::Promise::new(&mut |resolve, _| {
                    let global = js_sys::global();
                    let timeout = js_sys::Reflect::get(&global, &"setTimeout".into())
                        .unwrap()
                        .unchecked_into::<js_sys::Function>();
                    timeout
                        .call2(&global, &resolve, &milliseconds.into())
                        .unwrap();
                });
                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            })
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
#[cfg(not(target_arch = "wasm32"))]
fn runtime() -> &'static tokio::runtime::Handle {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
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

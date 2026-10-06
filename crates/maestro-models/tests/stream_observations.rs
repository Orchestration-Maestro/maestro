use maestro_models::*;
use std::sync::Arc;
use std::sync::Mutex;

fn queue() -> EventStream<i32> {
    EventStream::new(
        Arc::new(|value| Ok(*value < 0)),
        Arc::new(|value| Ok(*value)),
    )
}
fn poll<F: std::future::Future + ?Sized>(
    future: &mut std::pin::Pin<Box<F>>,
) -> std::task::Poll<F::Output> {
    future
        .as_mut()
        .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

#[test]
fn retained_read_keeps_delivery_after_all_handles_drop() {
    let stream = queue();
    let producer = stream.clone();
    let mut cursor = stream.iter();
    let mut read = cursor.next();
    producer.push(1).unwrap();
    drop(cursor);
    drop(stream);
    drop(producer);
    assert_eq!(poll(&mut read), std::task::Poll::Ready(Some(1)));
}

#[test]
fn retained_undelivered_read_stays_pending_after_all_handles_drop() {
    let stream = queue();
    let producer = stream.clone();
    let mut cursor = stream.iter();
    let mut read = cursor.next();
    drop(cursor);
    drop(stream);
    drop(producer);
    assert!(poll(&mut read).is_pending());
}

#[test]
fn abandoned_pending_reads_release_queue_and_wakers() {
    struct Tracked(std::sync::atomic::AtomicUsize);
    impl std::task::Wake for Tracked {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let tracked = Arc::new(Tracked(std::sync::atomic::AtomicUsize::new(0)));
    let weak_waker = Arc::downgrade(&tracked);
    let stream = queue();
    let mut cursor = stream.iter();
    let waker = std::task::Waker::from(tracked.clone());
    let mut cx = std::task::Context::from_waker(&waker);
    let mut first = cursor.next();
    assert!(first.as_mut().poll(&mut cx).is_pending());
    drop(first);
    let tracked_second = Arc::new(Tracked(std::sync::atomic::AtomicUsize::new(0)));
    let weak_second = Arc::downgrade(&tracked_second);
    let second_waker = std::task::Waker::from(tracked_second.clone());
    let mut second = cursor.next();
    assert!(
        second
            .as_mut()
            .poll(&mut std::task::Context::from_waker(&second_waker))
            .is_pending()
    );
    drop(second);
    drop(second_waker);
    drop(tracked_second);
    drop(cursor);
    drop(stream);
    drop(waker);
    drop(tracked);
    assert!(
        weak_waker.upgrade().is_none(),
        "abandoned reads must not retain the queue's registered wakers"
    );
    assert!(
        weak_second.upgrade().is_none(),
        "every abandoned observation waker must be released"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn result_wake_drains_terminal_event_before_eof() {
    struct Drain {
        cursor: Mutex<AsyncIterator<i32>>,
        observed: Mutex<Vec<Option<i32>>>,
    }
    impl std::task::Wake for Drain {
        fn wake(self: Arc<Self>) {
            let mut cursor = self.cursor.lock().unwrap();
            for _ in 0..2 {
                let std::task::Poll::Ready(value) = poll(&mut cursor.next()) else {
                    panic!("terminal iteration must be ready")
                };
                self.observed.lock().unwrap().push(value);
            }
        }
    }
    let stream = queue();
    let observer = Arc::new(Drain {
        cursor: Mutex::new(stream.iter()),
        observed: Mutex::new(vec![]),
    });
    let waker = std::task::Waker::from(observer.clone());
    let mut result = stream.result();
    assert!(
        result
            .as_mut()
            .poll(&mut std::task::Context::from_waker(&waker))
            .is_pending()
    );
    stream.push(-1).unwrap();
    assert_eq!(*observer.observed.lock().unwrap(), vec![Some(-1), None]);
    assert_eq!(poll(&mut result), std::task::Poll::Ready(-1));
}

#[test]
fn reentrant_terminal_extraction_observes_eof() {
    let holder = Arc::new(Mutex::new(None::<EventStream<i32>>));
    let observing = holder.clone();
    let stream = EventStream::new(
        Arc::new(|_: &i32| Ok(true)),
        Arc::new(move |value| {
            let stream = observing.lock().unwrap().as_ref().unwrap().clone();
            assert_eq!(
                poll(&mut stream.iter().next()),
                std::task::Poll::Ready(None)
            );
            Ok(*value)
        }),
    );
    *holder.lock().unwrap() = Some(stream.clone());
    stream.push(-1).unwrap();
    holder.lock().unwrap().take();
}

#[test]
fn preregistered_consumer_receives_terminal_then_eof() {
    let stream = queue();
    let mut cursor = stream.iter();
    let mut read = cursor.next();
    assert!(poll(&mut read).is_pending());
    stream.push(-1).unwrap();
    assert_eq!(poll(&mut read), std::task::Poll::Ready(Some(-1)));
    drop(read);
    assert_eq!(poll(&mut cursor.next()), std::task::Poll::Ready(None));
}

#[test]
fn exhausted_cursor_stays_exhausted_after_end_then_push() {
    let holder = Arc::new(Mutex::new(None::<EventStream<i32>>));
    let cursor = Arc::new(Mutex::new(None::<AsyncIterator<i32>>));
    let producer = holder.clone();
    let reader = cursor.clone();
    let stream = EventStream::new(
        Arc::new(move |_: &i32| {
            producer.lock().unwrap().as_ref().unwrap().end(None);
            assert_eq!(
                poll(&mut reader.lock().unwrap().as_mut().unwrap().next()),
                std::task::Poll::Ready(None)
            );
            Ok(false)
        }),
        Arc::new(|value| Ok(*value)),
    );
    *holder.lock().unwrap() = Some(stream.clone());
    *cursor.lock().unwrap() = Some(stream.iter());
    stream.push(7).unwrap();
    assert_eq!(
        poll(&mut cursor.lock().unwrap().as_mut().unwrap().next()),
        std::task::Poll::Ready(None)
    );
    assert_eq!(
        poll(&mut stream.iter().next()),
        std::task::Poll::Ready(Some(7))
    );
    holder.lock().unwrap().take();
}

#[test]
fn chained_cursor_registration_waits_for_consumer_poll() {
    let stream = queue();
    let mut a = stream.iter();
    drop(a.next());
    drop(a.next());
    stream.push(1).unwrap();
    let mut b = stream.iter();
    let mut second = b.next();
    stream.push(2).unwrap();
    assert_eq!(poll(&mut second), std::task::Poll::Ready(Some(2)));
}

#[path = "support/stream_interleavings.rs"]
mod stream_interleavings;

#[test]
fn cursor_interleavings_match_generator_observations() {
    stream_interleavings::check_corpus();
}

#[cfg(not(target_arch = "wasm32"))]
fn reentrant_waker(stream: EventStream<i32>, on_clone: bool) -> std::task::Waker {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{RawWaker, RawWakerVTable, Waker};
    struct Reentrant {
        stream: EventStream<i32>,
        on_clone: bool,
        fired: AtomicBool,
    }
    impl Reentrant {
        fn end(&self) {
            if !self.fired.swap(true, Ordering::SeqCst) {
                self.stream.end(None);
            }
        }
    }
    unsafe fn clone(data: *const ()) -> RawWaker {
        // The vtable pointer owns one Arc; borrowing it must not consume it.
        let state = std::mem::ManuallyDrop::new(unsafe { Arc::from_raw(data.cast::<Reentrant>()) });
        if state.on_clone {
            state.end();
        }
        RawWaker::new(Arc::into_raw(Arc::clone(&state)).cast(), &VTABLE)
    }
    unsafe fn wake(data: *const ()) {
        // Wake consumes the reference owned by this raw waker.
        let state = unsafe { Arc::from_raw(data.cast::<Reentrant>()) };
        if !state.on_clone {
            state.end();
        }
    }
    unsafe fn wake_by_ref(data: *const ()) {
        // A borrowed wake leaves the raw waker's reference intact.
        let state = std::mem::ManuallyDrop::new(unsafe { Arc::from_raw(data.cast::<Reentrant>()) });
        if !state.on_clone {
            state.end();
        }
    }
    unsafe fn drop(data: *const ()) {
        // Drop releases exactly the reference owned by this raw waker.
        std::mem::drop(unsafe { Arc::from_raw(data.cast::<Reentrant>()) });
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    let state = Arc::new(Reentrant {
        stream,
        on_clone,
        fired: AtomicBool::new(false),
    });
    // Every vtable operation maintains the Arc count and the state is Send + Sync.
    unsafe { Waker::from_raw(RawWaker::new(Arc::into_raw(state).cast(), &VTABLE)) }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pending_read_waker_clone_can_end_stream() {
    let (done, completed) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let stream = queue();
        let mut read = stream.iter().next();
        let waker = reentrant_waker(stream, true);
        assert_eq!(
            read.as_mut()
                .poll(&mut std::task::Context::from_waker(&waker)),
            std::task::Poll::Ready(None)
        );
        done.send(()).unwrap();
    });
    completed
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("waker clone must end the pending read without deadlocking");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pending_read_waker_wake_can_end_stream() {
    let (done, completed) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let stream = queue();
        let mut cursor = stream.iter();
        let mut read = cursor.next();
        let waker = reentrant_waker(stream.clone(), false);
        assert!(
            read.as_mut()
                .poll(&mut std::task::Context::from_waker(&waker))
                .is_pending()
        );
        stream.push(7).unwrap();
        assert_eq!(poll(&mut read), std::task::Poll::Ready(Some(7)));
        assert_eq!(poll(&mut cursor.next()), std::task::Poll::Ready(None));
        done.send(()).unwrap();
    });
    completed
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("waker wake must end the pending read without deadlocking");
}

#[cfg(not(target_arch = "wasm32"))]
fn check_reentrant_value(on_clone: bool) {
    struct Value {
        hook: Arc<dyn Fn() + Send + Sync>,
        on_clone: bool,
    }
    impl Clone for Value {
        fn clone(&self) -> Self {
            if self.on_clone {
                (self.hook)();
            }
            Self {
                hook: self.hook.clone(),
                on_clone: self.on_clone,
            }
        }
    }
    impl Drop for Value {
        fn drop(&mut self) {
            if !self.on_clone {
                (self.hook)();
            }
        }
    }
    let (done, completed) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let holder = Arc::new(Mutex::new(None::<EventStream<Value>>));
        let weak = Arc::downgrade(&holder);
        let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called = fired.clone();
        let hook = Arc::new(move || {
            if !called.swap(true, std::sync::atomic::Ordering::SeqCst) {
                let holder = weak.upgrade().unwrap();
                let stream = holder.lock().unwrap().as_ref().unwrap().clone();
                stream.end(None);
            }
        });
        let stream = EventStream::new(Arc::new(|_: &Value| Ok(false)), Arc::new(|v| Ok(v.clone())));
        *holder.lock().unwrap() = Some(stream.clone());
        let mut cursor = stream.iter();
        let mut read = cursor.next();
        stream.push(Value { hook, on_clone }).unwrap();
        let std::task::Poll::Ready(Some(value)) = poll(&mut read) else {
            panic!("value must be delivered")
        };
        drop(value);
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
        assert!(matches!(
            poll(&mut cursor.next()),
            std::task::Poll::Ready(None)
        ));
        holder.lock().unwrap().take();
        done.send(()).unwrap();
    });
    completed
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("caller value code must run outside stream locks");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn delivered_value_clone_can_end_stream() {
    check_reentrant_value(true);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn delivered_value_drop_can_end_stream() {
    check_reentrant_value(false);
}

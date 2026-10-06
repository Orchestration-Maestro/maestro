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

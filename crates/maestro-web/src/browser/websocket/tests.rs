//! Queue and weak callback ownership witnesses.
use super::*;
use std::{
    future::Future as _,
    task::{Context, Waker},
};

#[test]
fn socket_events_preserve_frames_across_reads() {
    let queue = Rc::new(RefCell::new(Queue::default()));
    let callback = Rc::downgrade(&queue);
    {
        let mut read = std::pin::pin!(next(&queue));
        assert!(
            read.as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        push(&callback, Event::Data(b"first".to_vec()));
    }
    push(&callback, Event::Data(b"second".to_vec()));
    assert!(matches!(ready(&queue), Event::Data(bytes) if bytes == b"first"));
    assert!(matches!(ready(&queue), Event::Data(bytes) if bytes == b"second"));
}

/// Poll an already queued event.
#[cfg(test)]
fn ready(queue: &Rc<RefCell<Queue>>) -> Event {
    let mut read = std::pin::pin!(next(queue));
    let Poll::Ready(event) = read.as_mut().poll(&mut Context::from_waker(Waker::noop())) else {
        panic!("expected queued event");
    };
    event
}

#[test]
fn socket_queue_handles_do_not_keep_owner_alive() {
    let queue = Rc::new(RefCell::new(Queue::default()));
    let callback = Rc::downgrade(&queue);
    drop(queue);
    push(&callback, Event::Open);
    assert!(callback.upgrade().is_none());
}

#[test]
fn socket_event_errors_and_close_settle_waiters() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::task::Wake;
    struct Counter(AtomicUsize);
    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let queue = Rc::new(RefCell::new(Queue::default()));
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&counter));
    {
        let mut read = std::pin::pin!(next(&queue));
        assert!(
            read.as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    let callback = Rc::downgrade(&queue);
    push(&callback, Event::Error(io::Error::other("native failure")));
    push(&callback, Event::Close);
    assert_eq!(counter.0.load(Ordering::SeqCst), 1);
    assert!(matches!(ready(&queue), Event::Error(error) if error.to_string() == "native failure"));
    assert!(matches!(ready(&queue), Event::Close));
}

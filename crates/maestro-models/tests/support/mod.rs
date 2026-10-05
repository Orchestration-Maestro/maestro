use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

struct Notifier(Thread);

impl Wake for Notifier {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(Notifier(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

#[test]
fn support_executor_resumes_a_woken_future() {
    struct WakeOnce(bool);
    impl Future for WakeOnce {
        type Output = u64;
        fn poll(mut self: std::pin::Pin<&mut Self>, context: &mut Context<'_>) -> Poll<u64> {
            if self.0 {
                return Poll::Ready(73);
            }
            self.0 = true;
            context.waker().wake_by_ref();
            Poll::Pending
        }
    }
    assert_eq!(block_on(WakeOnce(false)), 73);
}

pub mod conformance;

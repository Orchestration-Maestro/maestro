#[cfg(test)]
mod tests {
    use maestro_models::{Cancellation, EventStream};
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    #[derive(Default)]
    struct WakeCount(std::sync::atomic::AtomicUsize);
    impl Wake for WakeCount {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn poll<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    fn extraction_drops_reentrant_push() {
        let holder = Arc::new(std::sync::OnceLock::<EventStream<i32>>::new());
        let weak = Arc::downgrade(&holder);
        let stream = EventStream::new(
            |value: &i32| *value < 0,
            move |value| {
                if let Some(holder) = weak.upgrade()
                    && let Some(stream) = holder.get()
                {
                    stream.push(99);
                }
                *value
            },
        );
        assert!(holder.set(stream.clone()).is_ok());
        stream.push(-1);
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(Some(-1)));
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(None));
        assert_eq!(poll(std::pin::pin!(stream.result())), Poll::Ready(-1));
    }

    fn extraction_holds_terminal_publication() {
        let arrived = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let extract_arrived = Arc::clone(&arrived);
        let extract_release = Arc::clone(&release);
        let stream = EventStream::new(
            |_: &i32| true,
            move |value| {
                extract_arrived.wait();
                extract_release.wait();
                *value
            },
        );
        let producer = stream.clone();
        let thread = std::thread::spawn(move || producer.push(7));
        arrived.wait();
        stream.push(99);
        let mut reader = std::pin::pin!(stream.next());
        let mut result = std::pin::pin!(stream.result());
        assert_eq!(poll(reader.as_mut()), Poll::Pending);
        assert_eq!(poll(result.as_mut()), Poll::Pending);
        release.wait();
        assert!(thread.join().is_ok());
        assert_eq!(poll(reader.as_mut()), Poll::Ready(Some(7)));
        assert_eq!(poll(result.as_mut()), Poll::Ready(7));
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(None));
    }

    struct ReentrantWake(std::sync::Weak<std::sync::OnceLock<EventStream<i32>>>);
    impl ReentrantWake {
        fn end(&self) {
            if let Some(holder) = self.0.upgrade()
                && let Some(stream) = holder.get()
            {
                stream.end(None);
            }
        }
    }
    impl Wake for ReentrantWake {
        fn wake(self: Arc<Self>) {
            self.end();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.end();
        }
    }
    impl Drop for ReentrantWake {
        fn drop(&mut self) {
            self.end();
        }
    }

    fn waker_destruction_can_reenter() {
        let stream = EventStream::new(|_: &i32| false, |value| *value);
        let holder = Arc::new(std::sync::OnceLock::new());
        assert!(holder.set(stream.clone()).is_ok());
        let waker = Waker::from(Arc::new(ReentrantWake(Arc::downgrade(&holder))));
        let mut reader = std::pin::pin!(stream.next());
        assert_eq!(
            reader.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        );
        drop(waker);
        assert_eq!(poll(reader.as_mut()), Poll::Pending);
        assert_eq!(poll(reader.as_mut()), Poll::Ready(None));
    }

    struct ReentrantResult {
        holder: std::sync::Weak<std::sync::OnceLock<EventStream<i32, ReentrantResult>>>,
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl ReentrantResult {
        fn touch(&self) {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(holder) = self.holder.upgrade()
                && let Some(stream) = holder.get()
            {
                stream.end(None);
            }
        }
    }
    impl Clone for ReentrantResult {
        fn clone(&self) -> Self {
            self.touch();
            Self {
                holder: self.holder.clone(),
                calls: Arc::clone(&self.calls),
            }
        }
    }
    impl Drop for ReentrantResult {
        fn drop(&mut self) {
            self.touch();
        }
    }
    fn result_lifetime_can_reenter() {
        let holder = Arc::new(std::sync::OnceLock::new());
        let weak = Arc::downgrade(&holder);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let extract_calls = Arc::clone(&calls);
        let stream = EventStream::new(
            |_: &i32| true,
            move |_| ReentrantResult {
                holder: weak.clone(),
                calls: Arc::clone(&extract_calls),
            },
        );
        assert!(holder.set(stream.clone()).is_ok());
        stream.push(1);
        let Poll::Ready(result) = poll(std::pin::pin!(stream.result())) else {
            panic!("missing result")
        };
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        drop(result);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        stream.end(Some(ReentrantResult {
            holder: Arc::downgrade(&holder),
            calls: Arc::clone(&calls),
        }));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn maestro_dropped_observers_leave_producer_running() {
        waker_destruction_can_reenter();
        result_lifetime_can_reenter();
        let stream = EventStream::new(|value: &i32| *value < 0, |value| *value);
        let mut reader = Box::pin(stream.next());
        let mut result = Box::pin(stream.result());
        assert_eq!(poll(reader.as_mut()), Poll::Pending);
        assert_eq!(poll(result.as_mut()), Poll::Pending);
        drop((reader, result));
        let cancellation = Cancellation::new();
        let clone = cancellation.clone();
        let mut cancelled = std::pin::pin!(clone.cancelled());
        assert!(!clone.is_aborted());
        assert_eq!(poll(cancelled.as_mut()), Poll::Pending);
        cancellation.abort();
        cancellation.abort();
        assert!(clone.is_aborted());
        assert_eq!(poll(cancelled.as_mut()), Poll::Ready(()));
        assert_eq!(poll(std::pin::pin!(clone.cancelled())), Poll::Ready(()));
        stream.push(8);
        stream.push(-1);
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(Some(8)));
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(Some(-1)));
        assert_eq!(poll(std::pin::pin!(stream.result())), Poll::Ready(-1));
    }

    #[test]
    fn maestro_terminal_admission_is_atomic() -> Result<(), Box<dyn std::error::Error>> {
        extraction_drops_reentrant_push();
        extraction_holds_terminal_publication();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let classifier_barrier = Arc::clone(&barrier);
        let stream = EventStream::new(
            move |_: &i32| {
                classifier_barrier.wait();
                true
            },
            |value| *value,
        );
        let other = stream.clone();
        let thread = std::thread::spawn(move || other.push(11));
        stream.push(22);
        assert!(thread.join().is_ok());
        let event = poll(std::pin::pin!(stream.next()));
        assert!(matches!(event, Poll::Ready(Some(11 | 22))));
        let Poll::Ready(Some(event)) = event else {
            return Err("missing terminal event".into());
        };
        assert_eq!(poll(std::pin::pin!(stream.result())), Poll::Ready(event));
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(None));
        Ok(())
    }

    #[test]
    fn maestro_stream_end_keeps_first_result() {
        let stream = EventStream::new(|value: &i32| *value < 0, |value| *value);
        stream.push(1);
        let mut result = std::pin::pin!(stream.result());
        assert_eq!(poll(result.as_mut()), Poll::Pending);
        stream.end(None);
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(Some(1)));
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(None));
        assert_eq!(poll(result.as_mut()), Poll::Pending);
        stream.end(Some(7));
        stream.end(Some(9));
        stream.push(-4);
        assert_eq!(poll(result.as_mut()), Poll::Ready(7));
        assert_eq!(poll(std::pin::pin!(stream.result())), Poll::Ready(7));
    }

    #[test]
    fn maestro_stream_finishes_all_readers() {
        let stream = EventStream::new(|value: &i32| *value < 0, |value| *value);
        stream.push(1);
        stream.push(2);
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(Some(1)));
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(Some(2)));
        let mut first = std::pin::pin!(stream.next());
        let mut second = std::pin::pin!(stream.next());
        assert_eq!(poll(first.as_mut()), Poll::Pending);
        assert_eq!(poll(second.as_mut()), Poll::Pending);
        stream.push(3);
        assert_eq!(poll(second.as_mut()), Poll::Pending);
        assert_eq!(poll(first.as_mut()), Poll::Ready(Some(3)));
        let count = Arc::new(WakeCount::default());
        let waker = Waker::from(Arc::clone(&count));
        let mut context = Context::from_waker(&waker);
        assert_eq!(second.as_mut().poll(&mut context), Poll::Pending);
        let mut third = std::pin::pin!(stream.next());
        assert_eq!(third.as_mut().poll(&mut context), Poll::Pending);
        stream.push(-1);
        assert_eq!(count.0.load(std::sync::atomic::Ordering::Relaxed), 2);
        assert_eq!(poll(second.as_mut()), Poll::Ready(Some(-1)));
        assert_eq!(poll(third.as_mut()), Poll::Ready(None));
        stream.push(99);
        assert_eq!(poll(std::pin::pin!(stream.next())), Poll::Ready(None));
        assert_eq!(poll(std::pin::pin!(stream.result())), Poll::Ready(-1));
    }

    fn assistant_message() -> maestro_models::records::types::SharedAssistantMessage {
        Arc::new(std::sync::RwLock::new(serde_json::from_value(serde_json::json!({"role":"assistant","content":[],"api":"custom","provider":"custom","model":"m","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1})).unwrap()))
    }

    #[test]
    fn maestro_partial_messages_remain_shared() {
        use maestro_models::records::types::{
            AssistantMessageEvent as Event, AssistantMessageEventStream, DoneReason,
        };
        let message = assistant_message();
        let stream = AssistantMessageEventStream::new();
        for event in [
            Event::TextStart {
                content_index: 0,
                partial: Arc::clone(&message),
            },
            Event::ThinkingDelta {
                content_index: 1,
                delta: "reason".into(),
                partial: Arc::clone(&message),
            },
            Event::ToolcallDelta {
                content_index: 2,
                delta: "{}".into(),
                partial: Arc::clone(&message),
            },
        ] {
            stream.push(event);
        }
        let mut retained = Vec::new();
        for _ in 0..3 {
            let Poll::Ready(Some(event)) = poll(std::pin::pin!(stream.next())) else {
                panic!("missing update")
            };
            let (Event::TextStart { partial, .. }
            | Event::ThinkingDelta { partial, .. }
            | Event::ToolcallDelta { partial, .. }) = event
            else {
                panic!("unexpected update")
            };
            retained.push(partial);
        }
        message.write().unwrap().response_id = Some("later mutation".into());
        stream.push(Event::Done {
            reason: DoneReason::ToolUse,
            message: Arc::clone(&message),
        });
        let Poll::Ready(result) = poll(std::pin::pin!(stream.result())) else {
            panic!("missing result")
        };
        for partial in retained {
            assert!(Arc::ptr_eq(&partial, &result));
            assert_eq!(
                partial.read().unwrap().response_id.as_deref(),
                Some("later mutation")
            );
        }
        assert!(matches!(
            poll(std::pin::pin!(stream.next())),
            Poll::Ready(Some(Event::Done { .. }))
        ));
        assert!(matches!(
            poll(std::pin::pin!(stream.next())),
            Poll::Ready(None)
        ));
        assert!(Arc::ptr_eq(&result, &message));
    }

    static CLEANUPS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn maestro_cleanup_uses_live_registrations() {
        use maestro_models::records::session_resources::{
            SessionResourceCleanup, cleanup_session_resources, register_session_resource_cleanup,
        };
        let _serial = CLEANUPS.lock().unwrap();
        let visits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = Arc::clone(&visits);
        let first: SessionResourceCleanup = Arc::new(move |_| {
            log.lock().unwrap().push(1);
            Ok(())
        });
        let first_removal = register_session_resource_cleanup(Arc::clone(&first));
        let duplicate = register_session_resource_cleanup(Arc::clone(&first));
        let log = Arc::clone(&visits);
        let second: SessionResourceCleanup = Arc::new(move |_| {
            log.lock().unwrap().push(2);
            Ok(())
        });
        let second_removal = register_session_resource_cleanup(Arc::clone(&second));
        cleanup_session_resources(None).unwrap();
        assert_eq!(*visits.lock().unwrap(), [1, 2]);
        duplicate.remove();
        duplicate.remove();
        drop(register_session_resource_cleanup(Arc::clone(&first)));
        first_removal.remove();
        visits.lock().unwrap().clear();
        cleanup_session_resources(None).unwrap();
        assert_eq!(*visits.lock().unwrap(), [2]);
        second_removal.remove();
        drop((first_removal, duplicate, second_removal));
        assert_eq!(Arc::strong_count(&first), 1);
        assert_eq!(Arc::strong_count(&second), 1);
        for _ in 0..100 {
            let callback: SessionResourceCleanup = Arc::new(|_| Ok(()));
            let weak = Arc::downgrade(&callback);
            let removal = register_session_resource_cleanup(callback);
            removal.remove();
            drop(removal);
            assert!(weak.upgrade().is_none());
        }
    }

    #[test]
    fn maestro_cleanup_observes_live_changes() {
        use maestro_models::records::session_resources::{
            SessionResourceCleanup, SessionResourceRemoval, cleanup_session_resources,
            register_session_resource_cleanup,
        };
        let _serial = CLEANUPS.lock().unwrap();
        let visits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let unvisited = Arc::new(std::sync::Mutex::new(None::<SessionResourceRemoval>));
        let added = Arc::new(std::sync::Mutex::new(None::<SessionResourceRemoval>));
        let holder = Arc::new(std::sync::Mutex::new(None::<SessionResourceRemoval>));
        let log = Arc::clone(&visits);
        let extra: SessionResourceCleanup = Arc::new(move |_| {
            log.lock().unwrap().push("new");
            Ok(())
        });
        let (log, remove, keep, self_remove) = (
            Arc::clone(&visits),
            Arc::clone(&unvisited),
            Arc::clone(&added),
            Arc::clone(&holder),
        );
        let first = register_session_resource_cleanup(Arc::new(move |_| {
            log.lock().unwrap().push("first");
            remove.lock().unwrap().as_ref().unwrap().remove();
            *keep.lock().unwrap() = Some(register_session_resource_cleanup(Arc::clone(&extra)));
            self_remove.lock().unwrap().as_ref().unwrap().remove();
            cleanup_session_resources(Some("nested")).unwrap();
            Ok(())
        }));
        *holder.lock().unwrap() = Some(first);
        let log = Arc::clone(&visits);
        *unvisited.lock().unwrap() = Some(register_session_resource_cleanup(Arc::new(move |_| {
            log.lock().unwrap().push("removed");
            Ok(())
        })));
        cleanup_session_resources(None).unwrap();
        assert_eq!(*visits.lock().unwrap(), ["first", "new", "new"]);
        added.lock().unwrap().take().unwrap().remove();
        holder.lock().unwrap().take();
        unvisited.lock().unwrap().take();
    }
}

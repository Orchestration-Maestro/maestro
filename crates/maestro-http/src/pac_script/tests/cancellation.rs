//! Cancellation while a real native DNS helper is admitting its blocking lookup.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

#[test]
fn pac_abandoned_dns_invocation_releases_after_native_lookup() {
    let (started, ready) = oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let started = Mutex::new(Some(started));
    let released = Mutex::new(released);
    let threads = AtomicUsize::new(0);
    let released_token = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name_fn(move || {
            if threads.fetch_add(1, Ordering::SeqCst) == 1 {
                started.lock().unwrap().take().unwrap().send(()).unwrap();
                released.lock().unwrap().recv().unwrap();
            }
            "pac-native-lookup".into()
        })
        .build()
        .unwrap()
        .block_on(async {
            let resolver = super::super::Resolver::new(
                "function FindProxyForURL(){return dnsResolve('localhost')}".into(),
            )
            .await
            .unwrap();
            let commands = resolver.commands.as_ref().unwrap();
            let invocation = super::super::Invocation {
                commands: commands.clone(),
                token: Arc::new(()),
            };
            let weak = Arc::downgrade(&invocation.token);
            let (reply, result) = oneshot::channel();
            commands
                .send(super::super::Command::Resolve(super::super::Request {
                    token: invocation.token.clone(),
                    url: "https://example.com".into(),
                    reply,
                }))
                .unwrap();
            ready.await.unwrap();
            drop(result);
            drop(invocation);
            assert!(
                weak.upgrade().is_some(),
                "the native helper has not returned"
            );
            let worker = resolver.worker.as_ref().unwrap().abort_handle();
            drop(resolver);
            assert!(!worker.is_finished());
            release.send(()).unwrap();
            weak
        });
    assert!(released_token.upgrade().is_none());
}

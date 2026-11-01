//! Pending native input and retained callbacks outlive their initiating guest login.
use maestro_extensions::{ExtensionRunner, Local};
use maestro_models::{
    BoxFuture, Cancellation, OAuthAuthInfo, OAuthError, OAuthLoginCallbacks, OAuthPrompt,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// A native capture with an explicit prompt gate and final-release acknowledgment.
struct Pending {
    /// Live caller cancellation, never a sampled flag.
    signal: Cancellation,
    /// Only the first prompt waits for explicit native input.
    gate: Mutex<Option<oneshot::Receiver<String>>>,
    /// Announces that the actual native prompt has been polled.
    entered: mpsc::UnboundedSender<()>,
    /// Acknowledges destruction of the original native callback capture.
    dropped: Option<oneshot::Sender<()>>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        let _ = self.dropped.take().unwrap().send(());
    }
}
impl OAuthLoginCallbacks for Pending {
    fn on_auth(&self, _: OAuthAuthInfo) -> Result<(), OAuthError> {
        Ok(())
    }
    fn on_prompt(&self, _: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        let gate = self.gate.lock().unwrap().take();
        let entered = self.entered.clone();
        Box::pin(async move {
            entered.send(()).unwrap();
            Ok(match gate {
                Some(gate) => gate.await.unwrap(),
                None => "retained native answer".into(),
            })
        })
    }
    fn signal(&self) -> Option<&Cancellation> {
        Some(&self.signal)
    }
}

#[test]
fn oauth_guest_login_retains_pending_native_input_cancellation_and_captures() {
    super::super::super::components::local(async {
        let bus = super::super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::super::retained_components::loaded_on(bus.controller.for_extension())
                .await;
        super::super::call(&runner, "reverse install").await;
        let provider = maestro_models::get_oauth_provider("guest-reverse").unwrap();
        let (release, gate) = oneshot::channel();
        let (entered, mut ready) = mpsc::unbounded_channel();
        let (dropped, mut gone) = oneshot::channel();
        let signal = Cancellation::new();
        let callbacks = Arc::new(Pending {
            signal: signal.clone(),
            gate: Mutex::new(Some(gate)),
            entered,
            dropped: Some(dropped),
        });
        let login = provider.login(callbacks.clone(), None);
        drop(callbacks);
        drop(provider);
        let mut login = std::pin::pin!(login);
        tokio::select! { _ = &mut login => panic!("login did not wait for native input"), ready = ready.recv() => ready.unwrap(), }
        maestro_models::unregister_oauth_provider("guest-reverse");
        runner.invalidate(Some("original login context retired".into()));
        assert_eq!(
            super::super::call(&runner, "reverse inspect").await,
            json!([true, false])
        );
        signal.abort();
        assert_eq!(
            super::super::call(&runner, "reverse inspect").await,
            json!([true, true])
        );
        release.send("unblocked".into()).unwrap();
        let credentials = login.await.unwrap();
        assert_eq!(credentials.access, "unblocked");
        assert_eq!(credentials.extra["initialAborted"], false);
        assert_eq!(credentials.extra["aborted"], true);
        assert!(matches!(
            gone.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        retained_future(&runner, gone, &mut ready).await;
    });
}

/// A new input future, not the callback record, keeps the native capture alive.
async fn retained_future(
    runner: &ExtensionRunner<Local>,
    mut gone: oneshot::Receiver<()>,
    ready: &mut mpsc::UnboundedReceiver<()>,
) {
    assert_eq!(
        super::super::call(runner, "reverse pending-create").await,
        true
    );
    assert_eq!(
        super::super::call(runner, "reverse inspect").await,
        json!([false, null])
    );
    assert!(matches!(
        gone.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert_eq!(
        super::super::call(runner, "reverse pending-complete").await,
        "retained native answer"
    );
    ready.recv().await.unwrap();
    gone.await.unwrap();
    assert_eq!(super::super::call(runner, "reverse release").await, false);
}

#[test]
fn oauth_guest_login_preserves_prior_cancellation_and_releases_native_capture_once() {
    super::super::super::components::local(async {
        let bus = super::super::super::components::TracedBus::new();
        let (_owner, runner, _) =
            super::super::super::retained_components::loaded_on(bus.controller.for_extension())
                .await;
        super::super::call(&runner, "reverse install").await;
        let provider = maestro_models::get_oauth_provider("guest-reverse").unwrap();
        let (entered, _ready) = mpsc::unbounded_channel();
        let (dropped, mut gone) = oneshot::channel();
        let signal = Cancellation::new();
        signal.abort();
        let callbacks = Arc::new(Pending {
            signal,
            gate: Mutex::new(None),
            entered,
            dropped: Some(dropped),
        });
        let credentials = provider.login(callbacks, None).await.unwrap();
        assert_eq!(credentials.extra["initialAborted"], true);
        assert_eq!(credentials.extra["aborted"], true);
        assert_eq!(
            super::super::call(&runner, "reverse inspect").await,
            json!([true, true])
        );
        assert!(matches!(
            gone.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        maestro_models::unregister_oauth_provider("guest-reverse");
        drop(provider);
        assert_eq!(super::super::call(&runner, "reverse release").await, true);
        gone.await.unwrap();
        assert_eq!(super::super::call(&runner, "reverse release").await, false);
    });
}

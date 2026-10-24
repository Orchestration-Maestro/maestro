//! What a client releases when its work ends or its launch fails, observed in its own state.
#[cfg(unix)]
use super::process;
use super::{RpcClient, RpcClientError, RpcClientOptions};
#[cfg(unix)]
use std::cell::RefCell;
use std::{future::Future, rc::Rc};

/// Runs `body` on a local executor with its own runtime.
fn on_local<F: Future>(body: F) -> F::Output {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, body)
}

#[test]
fn failed_prompt_releases_its_collection_registration() {
    on_local(async {
        let client = RpcClient::new(RpcClientOptions::default());
        let outcome = client.prompt_and_wait("hello", None, None).await;
        assert!(matches!(outcome, Err(RpcClientError::NotStarted)));
        let live = {
            let mut listeners = client.inner.listeners.borrow_mut();
            let mut cursor = listeners.cursor();
            listeners.next_from(&mut cursor).is_some()
        };
        assert!(!live, "the collection's registration is removed");
        // The executor drops the cancelled deadline task on its next turn.
        tokio::task::yield_now().await;
        assert_eq!(
            Rc::strong_count(&client.inner),
            1,
            "nothing that watched for the collection's deadline remains"
        );
    });
}

#[cfg(unix)]
#[test]
fn failed_launch_publishes_nothing_and_leaves_no_child() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let source = scratch.path().join("peer.sh");
    std::fs::write(
        &source,
        "#!/bin/sh\necho launched >> \"$0.launches\"\necho '{\"type\": \"peer_ready\"}'\nexec cat > /dev/null\n",
    )
    .unwrap();
    let entry = scratch.path().join("peer");
    // Installing from a child process keeps the executable free of this process's writers.
    let installed = std::process::Command::new("install")
        .args(["-m", "0700"])
        .arg(&source)
        .arg(&entry)
        .status()
        .unwrap();
    assert!(installed.success());
    let options = RpcClientOptions {
        cli_path: Some(entry.to_str().unwrap().to_owned()),
        ..RpcClientOptions::default()
    };
    on_local(async {
        let client = RpcClient::new(options);
        let mut command = process::command_for(&client.options).unwrap();
        let refused = || Err(std::io::Error::other("no thread"));
        let error = process::launch(&client.inner, &mut command, refused)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "no thread");
        assert!(
            client.inner.state.borrow().running.is_none(),
            "a failed launch publishes no child"
        );
        let (ready, readied) = tokio::sync::oneshot::channel();
        let ready = RefCell::new(Some(ready));
        let _remove = client.on_event(Rc::new(move |_| {
            if let Some(ready) = ready.borrow_mut().take() {
                let _ = ready.send(());
            }
            Ok(())
        }));
        client.start().await.unwrap();
        readied.await.unwrap();
        client.stop().await.unwrap();
    });
    let launches = std::fs::read_to_string(format!("{}.launches", entry.display())).unwrap();
    assert_eq!(
        launches, "launched\n",
        "only the successful start launched a child"
    );
}

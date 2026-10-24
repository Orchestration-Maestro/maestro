//! What a client releases when its work ends, observed in its own state.
use super::{RpcClient, RpcClientError, RpcClientOptions};
use std::rc::Rc;

#[test]
fn failed_prompt_releases_its_collection_registration() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let client = RpcClient::new(RpcClientOptions::default());
        let outcome = client.prompt_and_wait("hello", None, None).await;
        assert!(matches!(outcome, Err(RpcClientError::NotStarted)));
        assert!(
            client.inner.listeners.borrow().next_from(0).is_none(),
            "the collection's registration is removed"
        );
        // The executor drops the cancelled deadline task on its next turn.
        tokio::task::yield_now().await;
        assert_eq!(
            Rc::strong_count(&client.inner),
            1,
            "nothing that watched for the collection's deadline remains"
        );
    });
}

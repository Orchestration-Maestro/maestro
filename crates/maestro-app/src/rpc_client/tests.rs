//! What a client releases when its work ends, observed in its own state.
use super::{RpcClient, RpcClientError, RpcClientOptions};
use std::{pin::pin, rc::Rc, task::Poll};

#[test]
fn failed_prompt_releases_its_collection_registration() {
    let client = RpcClient::new(RpcClientOptions::default());
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let outcome = pin!(client.prompt_and_wait("hello", None, None)).poll(&mut context);
    assert!(matches!(
        outcome,
        Poll::Ready(Err(RpcClientError::NotStarted))
    ));
    assert!(
        client.inner.listeners.borrow().next_from(0).is_none(),
        "the collection's registration is removed"
    );
    assert_eq!(
        Rc::strong_count(&client.inner),
        1,
        "nothing that watched for the collection's deadline remains"
    );
}

//! Synchronous progress acceptance with ordered owned delivery.
use crate::{
    AgentEvent, ToolProgress,
    agent::Inner,
    events::{Delivery, accept as accept_event},
};
use maestro_models::{Cancellation, ToolCall};
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinHandle};

pub(crate) fn accept(
    inner: Arc<Inner>,
    call: ToolCall,
    cancellation: Cancellation,
) -> (ToolProgress, JoinHandle<()>) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<Delivery>();
    let progress = Arc::new(move |partial_result| {
        let delivery = accept_event(
            &inner,
            AgentEvent::ToolExecutionUpdate {
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                args: call.arguments().expect("completed call").clone(),
                partial_result,
            },
        );
        let _ = sender.send(delivery);
    });
    let task = tokio::spawn(async move {
        while let Some(delivery) = receiver.recv().await {
            delivery.deliver(&cancellation).await;
        }
    });
    (progress, task)
}

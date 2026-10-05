//! Synchronous progress acceptance with independently driven owned deliveries.
use crate::{AgentEvent, ToolProgress, agent::Inner, events::accept as accept_event};
use maestro_models::{Cancellation, ToolCall};
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinHandle};

pub(crate) fn accept(
    inner: Arc<Inner>,
    call: ToolCall,
    cancellation: Cancellation,
) -> (ToolProgress, JoinHandle<()>) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<JoinHandle<()>>();
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
        let cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            delivery.deliver(&cancellation).await;
        });
        let _ = sender.send(task);
    });
    let task = tokio::spawn(async move {
        let mut failed = false;
        while let Some(delivery) = receiver.recv().await {
            failed |= delivery.await.is_err();
        }
        assert!(!failed, "progress subscriber failed");
    });
    (progress, task)
}

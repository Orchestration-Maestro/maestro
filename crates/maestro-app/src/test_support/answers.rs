//! Answers queued on the scripted provider: ones that record their request and one that
//! waits for its release.
use super::harness::Harness;
use super::records::{answer, message_text};
use maestro_models::{
    AssistantMessage, DiagnosticErrorInfo, Execution, FauxResponseInput, FauxResponseStep, Invoke,
    Local, ProviderExecution, Threaded,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// The answer of a scripted step, as either execution returns it.
type Answered<E> = <E as Execution>::Future<'static, Result<AssistantMessage, DiagnosticErrorInfo>>;

/// A scripted answer that records the request's message texts.
struct RecordPrompt {
    /// Prompts in request order.
    prompts: Arc<Mutex<Vec<String>>>,
    /// The answer.
    response: AssistantMessage,
}

impl RecordPrompt {
    /// Records the request and answers.
    fn respond<E: ProviderExecution>(
        &self,
        input: &FauxResponseInput<E>,
    ) -> std::future::Ready<Result<AssistantMessage, DiagnosticErrorInfo>> {
        let texts: Vec<String> = input
            .context
            .messages
            .iter()
            .map(|message| message_text(&json!(message)))
            .map(|text| text.as_str().unwrap_or_default().to_owned())
            .collect();
        maestro_lock::lock(&self.prompts).push(texts.join("\n---\n"));
        std::future::ready(Ok(self.response.clone()))
    }
}

impl Invoke<Local, FauxResponseInput<Local>> for RecordPrompt {
    type Return = Answered<Local>;
    fn invoke(&self, input: FauxResponseInput<Local>) -> Self::Return {
        Box::pin(self.respond(&input))
    }
}

impl Invoke<Threaded, FauxResponseInput<Threaded>> for RecordPrompt {
    type Return = Answered<Threaded>;
    fn invoke(&self, input: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(self.respond(&input))
    }
}

/// Queues the answers; the returned list collects each request's prompt.
pub fn script_responses<S>(
    h: &Harness<S>,
    responses: Vec<AssistantMessage>,
) -> Arc<Mutex<Vec<String>>> {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let steps = responses.into_iter().map(|response| {
        let prompts = Arc::clone(&prompts);
        FauxResponseStep::Factory(Arc::new(RecordPrompt { prompts, response }))
    });
    h.faux.set_responses(steps.collect());
    prompts
}

/// A scripted answer that reports its request on `requested` and answers once `release`
/// is notified, so the response streams until the test releases it.
struct HeldAnswer {
    /// Notified when the provider is asked.
    requested: Arc<Notify>,
    /// Releases the answer.
    release: Arc<Notify>,
    /// The answer.
    response: AssistantMessage,
}

impl HeldAnswer {
    /// Reports the request, then answers once released.
    fn respond(
        &self,
    ) -> impl Future<Output = Result<AssistantMessage, DiagnosticErrorInfo>> + Send + 'static {
        self.requested.notify_one();
        let release = Arc::clone(&self.release);
        let response = self.response.clone();
        async move {
            release.notified().await;
            Ok(response)
        }
    }
}

impl Invoke<Local, FauxResponseInput<Local>> for HeldAnswer {
    type Return = Answered<Local>;
    fn invoke(&self, _: FauxResponseInput<Local>) -> Self::Return {
        Box::pin(self.respond())
    }
}

impl Invoke<Threaded, FauxResponseInput<Threaded>> for HeldAnswer {
    type Return = Answered<Threaded>;
    fn invoke(&self, _: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(self.respond())
    }
}

/// The scripted steps: a held first answer `first`, then ordinary answers `rest`; returns
/// the request and release notifiers of the first.
pub fn hold_first_answer<S>(
    h: &Harness<S>,
    first: &str,
    rest: &[&str],
) -> (Arc<Notify>, Arc<Notify>) {
    let held = HeldAnswer {
        requested: Arc::default(),
        release: Arc::default(),
        response: answer(first, None, None),
    };
    let notifiers = (Arc::clone(&held.requested), Arc::clone(&held.release));
    let mut steps = vec![FauxResponseStep::Factory(Arc::new(held))];
    steps.extend(
        rest.iter()
            .map(|text| FauxResponseStep::Message(Box::new(answer(text, None, None)))),
    );
    h.faux.set_responses(steps);
    notifiers
}

//! A scripted HTTP transport that records when and how it was called.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use maestro_models::{
    AssistantMessage, AssistantMessageEventStream, BoxFuture, Context, Fetch, FetchError, HttpBody,
    HttpRequest, HttpResponse, Model, OpenAICompletionsOptions, StopReason, StreamOptions,
    stream_openai_completions,
};
use tokio::time::Instant;

use crate::chat::{TestResult, context, model};

/// How one attempt ends.
pub enum Attempt {
    /// Headers arrive at once; the body follows after `body_delay`.
    Respond {
        /// Status code.
        status: u16,
        /// Response headers.
        headers: Vec<(String, String)>,
        /// Body bytes.
        body: Vec<u8>,
        /// Time before the body arrives.
        body_delay: Duration,
    },
    /// The attempt fails.
    Fail(FetchError),
    /// Headers arrive, then reading the body fails.
    BrokenBody {
        /// Status code.
        status: u16,
        /// Failure while reading.
        error: FetchError,
    },
    /// The attempt never finishes.
    Hang,
    /// The attempt streams a body the test builds.
    Streaming {
        /// Status code.
        status: u16,
        /// Builds the body for each call.
        build: Arc<dyn Fn() -> HttpBody + Send + Sync>,
    },
}

impl Attempt {
    /// A complete event stream that ends immediately.
    pub fn success() -> Self {
        Self::status(200, &[])
    }

    /// An event stream that ends immediately, with the given status and headers.
    pub fn status(status: u16, headers: &[(&str, &str)]) -> Self {
        Self::body(status, headers, b"data: [DONE]\n\n".to_vec())
    }

    /// A body with the given status and headers.
    pub fn body(status: u16, headers: &[(&str, &str)], body: Vec<u8>) -> Self {
        Self::Respond {
            status,
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            body,
            body_delay: Duration::ZERO,
        }
    }

    /// Headers arrive at once and the body arrives after `delay`.
    pub fn slow_body(delay: Duration) -> Self {
        Self::Respond {
            status: 200,
            headers: Vec::new(),
            body: b"data: [DONE]\n\n".to_vec(),
            body_delay: delay,
        }
    }

    /// A successful response whose body is built by `build` for each call.
    pub fn streaming(build: impl Fn() -> HttpBody + Send + Sync + 'static) -> Self {
        Self::streaming_with_status(200, build)
    }

    /// A response with the given status whose body is built by `build` for each call.
    pub fn streaming_with_status(
        status: u16,
        build: impl Fn() -> HttpBody + Send + Sync + 'static,
    ) -> Self {
        Self::Streaming {
            status,
            build: Arc::new(build),
        }
    }

    /// The response or failure this attempt produces.
    fn respond(&self) -> BoxFuture<Result<HttpResponse, FetchError>> {
        let response = |status, headers, body| HttpResponse {
            status_text: String::new(),
            status,
            headers,
            body,
        };
        match self {
            Self::Fail(error) => Box::pin(std::future::ready(Err(error.clone()))),
            Self::Hang => Box::pin(std::future::pending()),
            Self::BrokenBody { status, error } => {
                let body: HttpBody = Box::pin(futures_util::stream::iter([Err(error.clone())]));
                Box::pin(std::future::ready(Ok(response(
                    *status,
                    BTreeMap::new(),
                    body,
                ))))
            }
            Self::Streaming { status, build } => {
                let body = build();
                Box::pin(std::future::ready(Ok(response(
                    *status,
                    BTreeMap::new(),
                    body,
                ))))
            }
            Self::Respond {
                status,
                headers,
                body,
                body_delay,
            } => {
                let (body, delay) = (body.clone(), *body_delay);
                let late: HttpBody = Box::pin(futures_util::stream::once(async move {
                    tokio::time::sleep(delay).await;
                    Ok(body)
                }));
                let headers = headers.iter().cloned().collect();
                Box::pin(std::future::ready(Ok(response(*status, headers, late))))
            }
        }
    }
}

/// The scripted transport and what it observed.
pub struct Transport {
    /// Transport to install in the options.
    pub fetch: Fetch,
    /// Start time of every attempt.
    pub starts: Arc<Mutex<Vec<Instant>>>,
    /// Every request received.
    pub requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Transport {
    /// Number of attempts started.
    pub fn attempts(&self) -> usize {
        self.starts.lock().map_or(0, |starts| starts.len())
    }

    /// Time between each attempt and the next.
    pub fn gaps(&self) -> Vec<Duration> {
        let starts = self.starts.lock().map(|s| s.clone()).unwrap_or_default();
        starts.windows(2).map(|pair| pair[1] - pair[0]).collect()
    }

    /// The URL and body of the first request.
    pub fn first_request(&self) -> TestResult<(String, Vec<u8>)> {
        let requests = self.requests.lock().map_err(|error| error.to_string())?;
        let first = requests.first().ok_or("no request")?;
        Ok((first.url.clone(), first.body.clone()))
    }
}

/// Answer the n-th call with the n-th attempt; the last attempt repeats.
pub fn transport(attempts: Vec<Attempt>) -> Transport {
    let starts = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (seen_starts, seen_requests) = (Arc::clone(&starts), Arc::clone(&requests));
    let fetch: Fetch = Arc::new(move |request| {
        let position = seen_starts.lock().map_or(0, |mut starts| {
            starts.push(Instant::now());
            starts.len() - 1
        });
        if let Ok(mut seen) = seen_requests.lock() {
            seen.push(request);
        }
        match attempts.get(position).or_else(|| attempts.last()) {
            Some(attempt) => attempt.respond(),
            None => Box::pin(std::future::ready(Err(FetchError::Aborted))),
        }
    });
    Transport {
        fetch,
        starts,
        requests,
    }
}

/// How a chat call ended.
pub struct Outcome {
    /// Final stop reason.
    pub stop_reason: StopReason,
    /// Final error text, if any.
    pub error: Option<String>,
}

/// Options, model and context for a plain call through `transport`.
pub fn call_inputs(transport: &Transport) -> TestResult<(Model, Context, StreamOptions)> {
    let options = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(Arc::clone(&transport.fetch)),
        ..StreamOptions::default()
    };
    let history = serde_json::json!({"messages": [{"role": "user", "content": "hello"}]});
    Ok((model(&serde_json::json!({}))?, context(&history)?, options))
}

/// Read every update's label, then the final message.
pub async fn drain(
    stream: &AssistantMessageEventStream,
) -> TestResult<(Vec<String>, AssistantMessage)> {
    let mut labels = Vec::new();
    while let Some(event) = stream.next().await {
        let label = serde_json::to_value(&event)?["type"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        labels.push(label);
    }
    let message = stream.result().await;
    let message = message.read().map_err(|error| error.to_string())?.clone();
    Ok((labels, message))
}

/// Run the call to its terminal update.
pub async fn finish(model: Model, context: Context, common: StreamOptions) -> TestResult<Outcome> {
    let options = OpenAICompletionsOptions {
        common,
        ..OpenAICompletionsOptions::default()
    };
    let (_, message) = drain(&stream_openai_completions(model, context, Some(options))).await?;
    Ok(Outcome {
        stop_reason: message.stop_reason,
        error: message.error_message,
    })
}

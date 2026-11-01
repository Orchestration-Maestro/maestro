//! Response and body ownership over the existing native HTTP transport.
use futures_util::{
    StreamExt,
    future::{Either, select},
};
use maestro_extensions::native_io::{IoFuture, Readable, Response};
use maestro_models::providers::http::{Raced, collect_body, race};
use maestro_models::{Cancellation, HttpBody, HttpResponse};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::Notify;

#[cfg(test)]
mod tests;

/// One original response with independently retained consuming body state.
pub(super) struct Http {
    /// Native status.
    status: u16,
    /// Original native reason phrase.
    status_text: String,
    /// Native HTTP owner's normalized response headers.
    headers: Vec<(String, String)>,
    /// Original body; a no-content response has no consumer.
    body: Option<Rc<Body>>,
}
impl Http {
    /// Retains the transport's stream without collecting or cloning bytes.
    pub(super) fn new(response: HttpResponse, signal: Option<Cancellation>) -> Self {
        Self {
            status: response.status,
            status_text: response.status_text,
            headers: response.headers.into_iter().collect(),
            body: (response.status != 204).then(|| {
                Rc::new(Body {
                    stream: RefCell::new(Some(response.body)),
                    signal,
                    ended: Cell::new(false),
                    used: Cell::new(false),
                    closed: Notify::new(),
                })
            }),
        }
    }
}
impl Response for Http {
    fn json(&self) -> IoFuture<'_, serde_json::Value> {
        Box::pin(async move {
            serde_json::from_str(&self.text().await?).map_err(|error| error.to_string())
        })
    }
    fn text(&self) -> IoFuture<'_, String> {
        Box::pin(async move {
            let bytes = match &self.body {
                Some(body) => body.collect().await?,
                None => Vec::new(),
            };
            Ok(maestro_text::decode_utf8(&bytes).into_owned())
        })
    }
    fn status(&self) -> u16 {
        self.status
    }
    fn status_text(&self) -> String {
        self.status_text.clone()
    }
    fn headers(&self) -> Vec<(String, String)> {
        self.headers.clone()
    }
    fn body(&self) -> Option<Rc<dyn Readable>> {
        self.body
            .as_ref()
            .map(|body| Rc::clone(body) as Rc<dyn Readable>)
    }
}
/// A single consuming stream; aliases never reset its cursor or caller signal.
struct Body {
    /// The stream is absent only while one read owns it or after settlement.
    stream: RefCell<Option<HttpBody>>,
    /// Original caller cancellation, also observed after headers arrive.
    signal: Option<Cancellation>,
    /// EOF, explicit close and a terminal error release the native stream.
    ended: Cell<bool>,
    /// Raw reading and whole-body consumption share this admission flag.
    used: Cell<bool>,
    /// Wakes a pending read when this consumer closes.
    closed: Notify,
}
/// Restores a raw read's stream when its future is dropped before settlement.
struct BodyRead<'a> {
    /// The consumer retains its independent close and terminal state.
    body: &'a Body,
    /// The stream claimed by this read, absent when another read already owns it.
    stream: Option<HttpBody>,
}
impl Drop for BodyRead<'_> {
    fn drop(&mut self) {
        if !self.body.ended.get() {
            *self.body.stream.borrow_mut() = self.stream.take();
        }
    }
}
impl Body {
    /// Claims an unread body once, retaining native cancellation and consumer close.
    async fn collect(&self) -> Result<Vec<u8>, String> {
        if self.used.replace(true) || self.ended.get() {
            return Err("body already used".into());
        }
        let stream = self
            .stream
            .borrow_mut()
            .take()
            .ok_or("body already in use")?;
        let reading = std::pin::pin!(race(collect_body(stream), None, self.signal.as_ref()));
        let closed = std::pin::pin!(self.closed.notified());
        let result = match select(reading, closed).await {
            Either::Left((Raced::Done(result), _)) => result.map_err(|e| e.to_string()),
            Either::Left((Raced::Cancelled | Raced::TimedOut, _)) => {
                Err(maestro_models::FetchError::Aborted.to_string())
            }
            Either::Right(_) => Err("body closed".into()),
        };
        self.ended.set(true);
        result
    }
    /// Races this consumer's next chunk against only its original signal and own close.
    async fn next(&self, stream: &mut HttpBody) -> Result<Option<Vec<u8>>, String> {
        let next = std::pin::pin!(race(stream.next(), None, self.signal.as_ref()));
        let closed = std::pin::pin!(self.closed.notified());
        match select(next, closed).await {
            Either::Left((Raced::Done(result), _)) => result.transpose().map_err(|e| e.to_string()),
            Either::Left((Raced::Cancelled | Raced::TimedOut, _)) => {
                Err(maestro_models::FetchError::Aborted.to_string())
            }
            Either::Right(_) => Ok(None),
        }
    }
}
impl Readable for Body {
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>> {
        Box::pin(async move {
            self.used.set(true);
            if self.ended.get() {
                return Ok(None);
            }
            let mut reading = BodyRead {
                body: self,
                stream: self.stream.borrow_mut().take(),
            };
            let stream = reading.stream.as_mut().ok_or("body already in use")?;
            let result = self.next(stream).await;
            if !matches!(result, Ok(Some(_))) {
                self.ended.set(true);
            }
            result
        })
    }
    fn close(&self) -> Result<(), String> {
        self.ended.set(true);
        self.stream.borrow_mut().take();
        self.closed.notify_waiters();
        Ok(())
    }
}

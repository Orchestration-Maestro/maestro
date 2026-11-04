//! One session-aware CDP connection without command deadlines.

use chromiumoxide::{
    Connection,
    cdp::{browser_protocol::target::SessionId, events::CdpEventMessage},
    types::{CallId, CdpJsonEventMessage, Message, Response},
};
use futures_util::StreamExt;
use serde_json::Value;
use std::{collections::HashMap, io};

/// Correlates responses while nested binding operations service the same socket.
pub(super) struct Dispatcher {
    /// Sole browser-control transport, absent once the connection ends.
    connection: Option<Connection<CdpEventMessage>>,
    /// Browser-close command awaiting acknowledgement.
    closing: Option<CallId>,
    /// Responses received while servicing another pending operation.
    responses: HashMap<CallId, Response>,
}

impl Dispatcher {
    /// Connect to the launched browser's advertised endpoint.
    pub async fn connect(url: &str) -> io::Result<Self> {
        Ok(Self {
            connection: Some(Connection::connect(url).await.map_err(io::Error::other)?),
            closing: None,
            responses: HashMap::new(),
        })
    }

    /// Queue one command using the library's own identifiers and session envelope.
    pub fn submit(
        &mut self,
        session: Option<&SessionId>,
        method: &'static str,
        params: Value,
    ) -> io::Result<CallId> {
        let id = self
            .connection
            .as_mut()
            .ok_or_else(disconnected)?
            .submit_command(method.into(), session.cloned(), params)
            .map_err(io::Error::other)?;
        if session.is_none() && method == "Browser.close" {
            self.closing = Some(id);
        }
        Ok(id)
    }

    /// Take the response for an outstanding operation, retaining other correlations.
    pub fn response(&mut self, id: CallId) -> Option<io::Result<Value>> {
        self.responses
            .remove(&id)
            .map(|response| match (response.error, response.result) {
                (Some(error), _) => Err(io::Error::other(error)),
                (None, Some(result)) => Ok(result),
                (None, None) => Err(io::Error::other("CDP response has no result")),
            })
    }

    /// Flush commands and read the next event, retaining intervening responses.
    pub async fn next(&mut self) -> io::Result<Option<CdpJsonEventMessage>> {
        match self
            .connection
            .as_mut()
            .ok_or_else(disconnected)?
            .next()
            .await
        {
            Some(Ok(Message::Response(response))) => {
                let closing = self.closing == Some(response.id);
                if closing {
                    self.closing = None;
                }
                if closing && response.error.is_none() && response.result.is_some() {
                    self.disconnect();
                }
                self.responses.insert(response.id, response);
                Ok(None)
            }
            Some(Ok(Message::Event(event))) => event.try_into().map(Some).map_err(io::Error::other),
            Some(Err(error)) => {
                self.disconnect();
                Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    format!("browser connection closed: {error}"),
                ))
            }
            None => {
                self.disconnect();
                Err(disconnected())
            }
        }
    }

    /// Retire the transport and pending correlations for every terminal outcome.
    fn disconnect(&mut self) {
        self.connection = None;
        self.closing = None;
        self.responses.clear();
    }
}

/// Report command admission on a connection that has already ended.
fn disconnected() -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, "browser connection closed")
}

#[cfg(test)]
mod tests;

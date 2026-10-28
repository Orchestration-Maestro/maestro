//! The API socket's connections.
use maestro_app::rpc_mode::{JsonlLineReader, RpcInput, RpcLineReader};
use std::{future::Future, io, pin::Pin};

/// A socket connection's lines.
pub(super) struct Admission<R> {
    /// The connection's lines.
    lines: JsonlLineReader<R>,
}

impl<R> Admission<R> {
    /// Admits a connection reading from `reader`.
    pub(super) fn new(reader: R) -> Self {
        Self {
            lines: JsonlLineReader::new(reader),
        }
    }
}

impl<R: tokio::io::AsyncRead + Unpin> RpcLineReader for Admission<R> {
    fn next_line(&mut self) -> Pin<Box<dyn Future<Output = io::Result<RpcInput>> + '_>> {
        self.lines.next_line()
    }

    fn detach(&mut self) {
        self.lines.detach();
    }
}

#[cfg(test)]
mod tests;

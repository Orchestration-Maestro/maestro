//! Framing and duplex byte forwarding for the sandbox's two proxy protocols.
use maestro_extensions_wasm::wasi;
use std::{
    future::{Future, poll_fn},
    task::Poll,
};
use wasi::sockets::types::TcpSocket;
use wit_bindgen::rt::async_support::{StreamReader, StreamResult, StreamWriter};

/// A peer's stream with bytes not yet consumed by its protocol handshake.
pub(crate) struct Reader {
    /// The remaining native stream.
    pub(crate) stream: StreamReader<u8>,
    /// Bytes already received beyond the parsed handshake.
    pub(crate) buffered: Vec<u8>,
}
impl Reader {
    /// Takes exactly the handshake's requested byte count.
    pub(crate) async fn take(&mut self, count: usize) -> Result<Vec<u8>, String> {
        while self.buffered.len() < count {
            self.fill().await?;
        }
        Ok(self.buffered.drain(..count).collect())
    }
    /// Takes one complete HTTP header, leaving a pipelined body untouched.
    pub(crate) async fn header(&mut self) -> Result<Vec<u8>, String> {
        loop {
            if let Some(index) = self
                .buffered
                .windows(4)
                .position(|bytes| bytes == b"\r\n\r\n")
            {
                return Ok(self.buffered.drain(..index + 4).collect());
            }
            self.fill().await?;
        }
    }
    /// Receives another native chunk, refusing an incomplete handshake at EOF.
    async fn fill(&mut self) -> Result<(), String> {
        self.buffered.extend(
            chunk(&mut self.stream)
                .await?
                .ok_or("Proxy peer closed before its request completed")?,
        );
        Ok(())
    }
}

/// Sends every byte or reports that its peer closed the stream early.
pub(crate) async fn write(writer: &mut StreamWriter<u8>, bytes: Vec<u8>) -> Result<(), String> {
    if writer.write_all(bytes).await.is_empty() {
        Ok(())
    } else {
        Err("Proxy peer closed while receiving data".into())
    }
}

/// Reads a batch without turning a byte stream into one allocation per byte.
async fn chunk(reader: &mut StreamReader<u8>) -> Result<Option<Vec<u8>>, String> {
    loop {
        let (status, bytes) = reader.read(Vec::with_capacity(8192)).await;
        if !bytes.is_empty() {
            return Ok(Some(bytes));
        }
        match status {
            StreamResult::Complete(_) => {}
            StreamResult::Dropped => return Ok(None),
            StreamResult::Cancelled => return Err("Proxy stream read was cancelled".into()),
        }
    }
}

/// Copies one direction until EOF, then half-closes only its destination's output.
async fn copy(
    mut reader: StreamReader<u8>,
    mut writer: StreamWriter<u8>,
    initial: Vec<u8>,
) -> Result<(), String> {
    if !initial.is_empty() {
        write(&mut writer, initial).await?;
    }
    while let Some(bytes) = chunk(&mut reader).await? {
        write(&mut writer, bytes).await?;
    }
    Ok(())
}

/// Forwards both directions concurrently, retaining each socket through both half-closes.
pub(crate) async fn tunnel(
    reader: Reader,
    writer: StreamWriter<u8>,
    destination: TcpSocket,
    prefix: Vec<u8>,
) -> Result<(), String> {
    let (incoming, received) = destination.receive();
    let (outgoing, bytes) = wasi::wit_stream::new::<u8>();
    let sent = destination.send(bytes);
    let mut initial = prefix;
    initial.extend(reader.buffered);
    let mut upload = std::pin::pin!(copy(reader.stream, outgoing, initial));
    let mut download = std::pin::pin!(copy(incoming, writer, Vec::new()));
    let (mut uploaded, mut downloaded) = (false, false);
    poll_fn(|cx| {
        if !uploaded && let Poll::Ready(result) = upload.as_mut().poll(cx) {
            result?;
            uploaded = true;
        }
        if !downloaded && let Poll::Ready(result) = download.as_mut().poll(cx) {
            result?;
            downloaded = true;
        }
        if uploaded && downloaded {
            Poll::Ready(Ok::<_, String>(()))
        } else {
            Poll::Pending
        }
    })
    .await?;
    received.await.map_err(|error| format!("{error:?}"))?;
    sent.await.map_err(|error| format!("{error:?}"))
}

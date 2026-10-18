//! Protocol replies.

/// Image command reply.
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    /// Id.
    pub id: u32,
    /// Image number.
    pub image_number: u32,
    /// Placement id.
    pub placement_id: u32,
    /// Message.
    pub message: Vec<u8>,
}

impl Default for Response {
    fn default() -> Self {
        Self {
            id: 0,
            image_number: 0,
            placement_id: 0,
            message: b"OK".to_vec(),
        }
    }
}
impl Response {
    /// Write an identity-bearing reply, with keys in protocol order.
    ///
    /// # Errors
    /// Propagates the writer's original error.
    pub fn encode<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        if self.empty() {
            return Ok(());
        }
        writer.write_all(b"\x1b_G")?;
        let mut separator = "";
        for (key, value) in [
            ('i', self.id),
            ('I', self.image_number),
            ('p', self.placement_id),
        ] {
            if value == 0 {
                continue;
            }
            write!(writer, "{separator}{key}={value}")?;
            separator = ",";
        }
        writer.write_all(b";")?;
        writer.write_all(&self.message)?;
        writer.write_all(b"\x1b\\")
    }
    /// Whether the reply is exactly the success message.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.message == b"OK"
    }
    /// Whether both image identifiers are absent, irrespective of placement.
    #[must_use]
    pub fn empty(&self) -> bool {
        self.id == 0 && self.image_number == 0
    }
}

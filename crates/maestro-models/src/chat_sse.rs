//! Incremental SSE byte framing with strict UTF-8 and final-frame validation.
use crate::Failure;
#[derive(Default)]
pub(crate) struct Sse {
    line: Vec<u8>,
    data: Vec<String>,
    has_data: bool,
    after_cr: bool,
}
impl Sse {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, Failure> {
        let mut frames = Vec::new();
        for &b in bytes {
            if self.after_cr {
                self.after_cr = false;
                if b == b'\n' {
                    continue;
                }
            }
            if b == b'\r' || b == b'\n' {
                self.line(&mut frames)?;
                self.after_cr = b == b'\r';
            } else {
                self.line.push(b)
            }
        }
        Ok(frames)
    }
    fn line(&mut self, frames: &mut Vec<String>) -> Result<(), Failure> {
        let line = String::from_utf8(std::mem::take(&mut self.line))
            .map_err(|_| Failure::MalformedStream)?;
        if line.is_empty() {
            if self.has_data {
                frames.push(self.data.join("\n"));
            }
            self.data.clear();
            self.has_data = false;
            return Ok(());
        }
        if line.starts_with(':') {
            return Ok(());
        }
        let (field, value) = line.split_once(':').unwrap_or((&line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        if field == "data" {
            self.has_data = true;
            self.data.push(value.into());
        }
        Ok(())
    }
    pub fn end(&self) -> Result<(), Failure> {
        if !self.line.is_empty() || self.has_data {
            Err(Failure::MalformedStream)
        } else {
            Ok(())
        }
    }
}

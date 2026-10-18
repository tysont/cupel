// ABOUTME: Text a run writes in pieces, handed on whole lines at a time: every complete line written so far when a
// ABOUTME: write ends one, and a last line without a newline when the writer is dropped. The daemon and MCP share it.
use std::io::{self, Write};

/// A writer that hands `send` each run of complete lines, newlines included, as it is written.
pub(crate) struct Lines<F: FnMut(&str) -> io::Result<()>> {
    pending: Vec<u8>,
    send: F,
}

impl<F: FnMut(&str) -> io::Result<()>> Lines<F> {
    pub(crate) fn new(send: F) -> Self {
        Self {
            pending: Vec::new(),
            send,
        }
    }
}

impl<F: FnMut(&str) -> io::Result<()>> Write for Lines<F> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        if let Some(end) = self.pending.iter().rposition(|&byte| byte == b'\n') {
            let lines: Vec<u8> = self.pending.drain(..=end).collect();
            (self.send)(&String::from_utf8_lossy(&lines))?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<F: FnMut(&str) -> io::Result<()>> Drop for Lines<F> {
    fn drop(&mut self) {
        if !self.pending.is_empty() {
            let rest = std::mem::take(&mut self.pending);
            let _ = (self.send)(&String::from_utf8_lossy(&rest));
        }
    }
}

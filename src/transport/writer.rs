/// Little-endian append cursor over a growing byte buffer.
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    #[must_use]
    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            buf: Vec::with_capacity(bytes),
        }
    }

    pub fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i64(&mut self, value: i64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Write a length-prefixed string. A string longer than `u32::MAX` cannot
    /// occur here: the ring rejects frames far smaller than that first.
    pub fn str(&mut self, value: &str) {
        let bytes = value.as_bytes();
        self.u32(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
        self.buf.extend_from_slice(bytes);
    }

    /// Append raw bytes with no length prefix.
    ///
    /// The caller must make the length recoverable — either the payload runs to
    /// the end of the frame (see [`Reader::rest`](super::Reader::rest)) or a
    /// prefix was written first.
    pub fn bytes(&mut self, value: &[u8]) {
        self.buf.extend_from_slice(value);
    }

    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

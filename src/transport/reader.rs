/// Little-endian read cursor over a borrowed frame.
///
/// Every accessor is bounds-checked and returns `None` on a short or malformed
/// frame, so a corrupt ring cannot panic the drain.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(len)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    pub fn u8(&mut self) -> Option<u8> {
        let raw: [u8; 1] = self.take(1)?.try_into().ok()?;
        Some(raw[0])
    }

    pub fn u32(&mut self) -> Option<u32> {
        let raw: [u8; 4] = self.take(4)?.try_into().ok()?;
        Some(u32::from_le_bytes(raw))
    }

    pub fn i64(&mut self) -> Option<i64> {
        let raw: [u8; 8] = self.take(8)?.try_into().ok()?;
        Some(i64::from_le_bytes(raw))
    }

    /// Read a length-prefixed UTF-8 string.
    ///
    /// This is the one unavoidable validation: the bytes crossed a thread
    /// boundary, so they are untrusted and `String` must stay UTF-8. It is a
    /// linear scan, not a parse — no allocation per field beyond the copy, no
    /// escape handling, no structural backtracking.
    pub fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        self.string_of(len)
    }

    /// Read a string body whose length prefix the caller already consumed.
    pub fn string_of(&mut self, len: usize) -> Option<String> {
        let slice = self.take(len)?;
        String::from_utf8(slice.to_vec()).ok()
    }

    /// Every byte from the cursor to the end, consuming them.
    ///
    /// For a payload that runs to the end of the frame, so it needs no length
    /// prefix of its own.
    pub fn rest(&mut self) -> &'a [u8] {
        let slice = &self.bytes[self.at..];
        self.at = self.bytes.len();
        slice
    }

    /// True when every byte has been consumed. A decoder checks this to reject
    /// a frame with trailing garbage.
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.at == self.bytes.len()
    }
}

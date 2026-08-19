//! A flat binary encoding of [`NostrNote`], for moving notes through a shared
//! byte buffer without paying a JSON parse on the consumer.
//!
//! # Why this exists
//!
//! The SAB transport moves bytes, so something must turn bytes back into a
//! note. Sending the relay's raw `["EVENT",..]` text makes that step a full
//! JSON parse, and it runs on the UI thread, in bursts of up to one drain
//! quota per animation frame. Measured against the shared-linear-memory ring
//! at 2900 notes/s, that parse cost 11% jank against 0.1%.
//!
//! The producer already parsed the note to dedup and filter it. This encoding
//! keeps that work: the worker writes the parsed fields out flat, and the
//! consumer reads them back with bounds-checked slicing and one UTF-8
//! validation per string. No tokenizer, no escape decoding, no field-name
//! matching, no `serde` machinery.
//!
//! # Frame layout
//!
//! All integers are little-endian. Strings are a `u32` byte length followed by
//! that many UTF-8 bytes. Optional strings use [`ABSENT`] as the length.
//!
//! ```text
//! created_at : i64
//! kind       : u32
//! pubkey     : str
//! content    : str
//! id         : str?
//! sig        : str?
//! tag_rows   : u32, then per row: u32 cell count, then each cell as str
//! ```
//!
//! The field order is fixed and both sides are compiled together, so there is
//! no version negotiation. This is a transport-internal format, never written
//! to disk and never sent over a network.

use super::{Reader, Writer};
use nostro2::{NostrNote, NostrTags};

/// Length marker for an absent `Option<String>`. No real string reaches
/// `u32::MAX` bytes: the ring refuses frames orders of magnitude smaller.
const ABSENT: u32 = u32::MAX;

/// A field the frame may legitimately omit.
///
/// Distinct from a decode failure: a malformed frame is the `None` of the
/// enclosing `Option`, an absent field is [`Self::Absent`].
enum OptionalField {
    Absent,
    Present(String),
}

impl OptionalField {
    fn into_option(self) -> Option<String> {
        match self {
            Self::Absent => None,
            Self::Present(value) => Some(value),
        }
    }
}

/// Encodes and decodes [`NostrNote`] as a flat frame.
pub struct NoteCodec;

impl NoteCodec {
    /// Estimated frame size, to size the buffer in one allocation.
    fn size_hint(note: &NostrNote) -> usize {
        let strings = note.pubkey.len()
            + note.content.len()
            + note.id.as_ref().map_or(0, String::len)
            + note.sig.as_ref().map_or(0, String::len);
        let tags: usize = note
            .tags
            .iter()
            .map(|row| 4 + row.iter().map(|c| 4 + c.len()).sum::<usize>())
            .sum();
        strings + tags + 64
    }

    #[must_use]
    pub fn encode(note: &NostrNote) -> Vec<u8> {
        let mut w = Writer::with_capacity(Self::size_hint(note));
        w.i64(note.created_at);
        w.u32(note.kind);
        w.str(&note.pubkey);
        w.str(&note.content);
        Self::write_option(&mut w, note.id.as_deref());
        Self::write_option(&mut w, note.sig.as_deref());
        w.u32(u32::try_from(note.tags.len()).unwrap_or(0));
        for row in note.tags.iter() {
            w.u32(u32::try_from(row.len()).unwrap_or(0));
            for cell in row {
                w.str(cell);
            }
        }
        w.finish()
    }

    /// Decode a frame. Returns `None` for a truncated, malformed, or non-UTF-8
    /// frame rather than panicking.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<NostrNote> {
        let mut r = Reader::new(bytes);
        let created_at = r.i64()?;
        let kind = r.u32()?;
        let pubkey = r.string()?;
        let content = r.string()?;
        let id = Self::read_option(&mut r)?.into_option();
        let sig = Self::read_option(&mut r)?.into_option();
        let mut tags = NostrTags::new();
        let rows = r.u32()?;
        for _ in 0..rows {
            let cells = r.u32()?;
            let mut row = Vec::with_capacity(cells as usize);
            for _ in 0..cells {
                row.push(r.string()?);
            }
            tags.add_row(row);
        }
        if !r.is_done() {
            return None;
        }
        Some(NostrNote {
            pubkey,
            created_at,
            kind,
            tags,
            content,
            id,
            sig,
        })
    }

    fn write_option(w: &mut Writer, value: Option<&str>) {
        match value {
            Some(s) => w.str(s),
            None => w.u32(ABSENT),
        }
    }

    fn read_option(r: &mut Reader) -> Option<OptionalField> {
        match r.u32()? {
            ABSENT => Some(OptionalField::Absent),
            len => Some(OptionalField::Present(r.string_of(len as usize)?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NoteCodec, NostrNote, NostrTags};

    struct NoteBuilder;

    impl NoteBuilder {
        fn plain() -> NostrNote {
            NostrNote {
                pubkey: "a".repeat(64),
                created_at: 1_700_000_000,
                kind: 1,
                tags: NostrTags::new(),
                content: "hello".to_string(),
                id: Some("b".repeat(64)),
                sig: Some("c".repeat(128)),
            }
        }

        fn tagged() -> NostrNote {
            let mut note = Self::plain();
            note.tags.add_event_tag(&"d".repeat(64));
            note.tags.add_pubkey_tag(&"e".repeat(64), Some("wss://r.io"));
            note.tags
                .add_row(vec!["custom".into(), "1".into(), "2".into()]);
            note
        }
    }

    #[test]
    fn plain_note_round_trips() {
        let note = NoteBuilder::plain();
        assert_eq!(NoteCodec::decode(&NoteCodec::encode(&note)), Some(note));
    }

    #[test]
    fn tags_round_trip_with_row_shape_intact() {
        let note = NoteBuilder::tagged();
        let back = NoteCodec::decode(&NoteCodec::encode(&note)).expect("decodes");
        assert_eq!(back.tags.len(), 3);
        assert_eq!(back.tags.row(2).map(<[String]>::len), Some(3));
        assert_eq!(back, note);
    }

    #[test]
    fn absent_id_and_sig_stay_absent() {
        let mut note = NoteBuilder::plain();
        note.id = None;
        note.sig = None;
        let back = NoteCodec::decode(&NoteCodec::encode(&note)).expect("decodes");
        assert!(back.id.is_none());
        assert!(back.sig.is_none());
        assert_eq!(back, note);
    }

    #[test]
    fn empty_string_is_distinct_from_absent() {
        let mut note = NoteBuilder::plain();
        note.id = Some(String::new());
        let back = NoteCodec::decode(&NoteCodec::encode(&note)).expect("decodes");
        assert_eq!(back.id, Some(String::new()));
    }

    #[test]
    fn multibyte_content_survives() {
        let mut note = NoteBuilder::plain();
        note.content = "日本語 🐾 emoji".to_string();
        assert_eq!(NoteCodec::decode(&NoteCodec::encode(&note)), Some(note));
    }

    #[test]
    fn truncated_frame_is_rejected() {
        let frame = NoteCodec::encode(&NoteBuilder::tagged());
        for cut in 0..frame.len() {
            assert_eq!(NoteCodec::decode(&frame[..cut]), None, "cut at {cut}");
        }
    }

    #[test]
    fn trailing_garbage_is_rejected() {
        let mut frame = NoteCodec::encode(&NoteBuilder::plain());
        frame.push(0);
        assert_eq!(NoteCodec::decode(&frame), None);
    }

    #[test]
    fn invalid_utf8_is_rejected_without_panicking() {
        let mut note = NoteBuilder::plain();
        note.content = "ascii".to_string();
        let mut frame = NoteCodec::encode(&note);
        let at = frame
            .windows(5)
            .position(|w| w == b"ascii")
            .expect("content present");
        frame[at] = 0xff;
        assert_eq!(NoteCodec::decode(&frame), None);
    }

    #[test]
    fn absurd_length_prefix_is_rejected() {
        let mut frame = NoteCodec::encode(&NoteBuilder::plain());
        let pubkey_len_at = 8 + 4;
        frame[pubkey_len_at..pubkey_len_at + 4].copy_from_slice(&1_000_000u32.to_le_bytes());
        assert_eq!(NoteCodec::decode(&frame), None);
    }
}

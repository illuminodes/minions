//! One tagged frame format for EVERY message the relay pool sends, in both
//! directions.
//!
//! # Why this exists
//!
//! The shared ring moves opaque bytes. If only notes travelled through it, the
//! pool would need a second channel for commands, relay events, and health —
//! two transports live at once, each with its own ordering and failure mode.
//!
//! So the transport choice is made once at boot and then everything rides the
//! same path. That requires a frame that can carry any message, which is this
//! module: a `u8` discriminant followed by the body.
//!
//! # Ordering
//!
//! A single tagged stream also preserves the order BETWEEN message kinds, which
//! two parallel channels cannot. That matters: `RelayHealth` saying a socket
//! closed must not overtake the notes that socket already delivered, and a
//! `Close` must not overtake the `Subscribe` it closes.
//!
//! # Layout
//!
//! ```text
//! tag  : u8
//! body : depends on tag
//! ```
//!
//! Notes are the hot path and use [`NoteCodec`]'s flat encoding. The rare
//! control messages carry their strings directly, so no JSON parse happens on
//! either thread for any message kind.

use super::{NoteCodec, Reader, Writer};
use nostro2::NostrNote;

/// Frame discriminants. Values are explicit because they are written to bytes;
/// changing one is a wire break, not a refactor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum Tag {
    Note = 1,
    RelayEvent = 2,
    RelayHealth = 3,
    Connect = 4,
    AddRelay = 5,
    RemoveRelay = 6,
    Subscribe = 7,
    Close = 8,
    Send = 9,
}

impl Tag {
    const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::Note),
            2 => Some(Self::RelayEvent),
            3 => Some(Self::RelayHealth),
            4 => Some(Self::Connect),
            5 => Some(Self::AddRelay),
            6 => Some(Self::RemoveRelay),
            7 => Some(Self::Subscribe),
            8 => Some(Self::Close),
            9 => Some(Self::Send),
            _ => None,
        }
    }
}

/// A message travelling from the worker to the application.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outbound {
    /// A matched note, already parsed by the worker.
    Note(NostrNote),
    /// A non-note relay frame (EOSE / OK / NOTICE / AUTH / CLOSED), verbatim.
    RelayEvent(String),
    /// Per-relay connection state, as `(url, ready_state)` pairs. The state is
    /// a `u8` here so this module stays free of the web bindings; the pool maps
    /// it back to its own enum.
    RelayHealth(Vec<(String, u8)>),
}

/// A message travelling from the application to the worker.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Inbound {
    Connect(Vec<String>),
    AddRelay(String),
    RemoveRelay(String),
    Subscribe {
        sub_id: String,
        filter_json: String,
        req_json: String,
    },
    Close(String),
    Send(String),
}

/// Encodes and decodes both directions of the tagged stream.
pub struct Envelope;

impl Envelope {
    #[must_use]
    pub fn encode_outbound(message: &Outbound) -> Vec<u8> {
        let mut w = Writer::with_capacity(128);
        match message {
            Outbound::Note(note) => {
                w.u8(Tag::Note as u8);
                w.bytes(&NoteCodec::encode(note));
            }
            Outbound::RelayEvent(raw) => {
                w.u8(Tag::RelayEvent as u8);
                w.str(raw);
            }
            Outbound::RelayHealth(updates) => {
                w.u8(Tag::RelayHealth as u8);
                w.u32(u32::try_from(updates.len()).unwrap_or(0));
                for (url, state) in updates {
                    w.str(url);
                    w.u8(*state);
                }
            }
        }
        w.finish()
    }

    #[must_use]
    pub fn decode_outbound(bytes: &[u8]) -> Option<Outbound> {
        let mut r = Reader::new(bytes);
        let tag = Tag::from_byte(r.u8()?)?;
        let message = match tag {
            Tag::Note => Outbound::Note(NoteCodec::decode(r.rest())?),
            Tag::RelayEvent => Outbound::RelayEvent(Self::last_string(&mut r)?),
            Tag::RelayHealth => {
                let count = r.u32()?;
                let mut updates = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let url = r.string()?;
                    updates.push((url, r.u8()?));
                }
                if !r.is_done() {
                    return None;
                }
                Outbound::RelayHealth(updates)
            }
            _ => return None,
        };
        Some(message)
    }

    #[must_use]
    pub fn encode_inbound(message: &Inbound) -> Vec<u8> {
        let mut w = Writer::with_capacity(128);
        match message {
            Inbound::Connect(urls) => {
                w.u8(Tag::Connect as u8);
                w.u32(u32::try_from(urls.len()).unwrap_or(0));
                for url in urls {
                    w.str(url);
                }
            }
            Inbound::AddRelay(url) => {
                w.u8(Tag::AddRelay as u8);
                w.str(url);
            }
            Inbound::RemoveRelay(url) => {
                w.u8(Tag::RemoveRelay as u8);
                w.str(url);
            }
            Inbound::Subscribe {
                sub_id,
                filter_json,
                req_json,
            } => {
                w.u8(Tag::Subscribe as u8);
                w.str(sub_id);
                w.str(filter_json);
                w.str(req_json);
            }
            Inbound::Close(sub_id) => {
                w.u8(Tag::Close as u8);
                w.str(sub_id);
            }
            Inbound::Send(event_json) => {
                w.u8(Tag::Send as u8);
                w.str(event_json);
            }
        }
        w.finish()
    }

    #[must_use]
    pub fn decode_inbound(bytes: &[u8]) -> Option<Inbound> {
        let mut r = Reader::new(bytes);
        let tag = Tag::from_byte(r.u8()?)?;
        let message = match tag {
            Tag::Connect => {
                let count = r.u32()?;
                let mut urls = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    urls.push(r.string()?);
                }
                if !r.is_done() {
                    return None;
                }
                Inbound::Connect(urls)
            }
            Tag::AddRelay => Inbound::AddRelay(Self::last_string(&mut r)?),
            Tag::RemoveRelay => Inbound::RemoveRelay(Self::last_string(&mut r)?),
            Tag::Subscribe => {
                let sub_id = r.string()?;
                let filter_json = r.string()?;
                let req_json = r.string()?;
                if !r.is_done() {
                    return None;
                }
                Inbound::Subscribe {
                    sub_id,
                    filter_json,
                    req_json,
                }
            }
            Tag::Close => Inbound::Close(Self::last_string(&mut r)?),
            Tag::Send => Inbound::Send(Self::last_string(&mut r)?),
            _ => return None,
        };
        Some(message)
    }

    /// Read a string that must be the final field, rejecting trailing bytes.
    fn last_string(r: &mut Reader) -> Option<String> {
        let value = r.string()?;
        r.is_done().then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{Envelope, Inbound, Outbound};
    use nostro2::{NostrNote, NostrTags};

    struct Sample;

    impl Sample {
        fn note() -> NostrNote {
            let mut tags = NostrTags::new();
            tags.add_event_tag(&"d".repeat(64));
            NostrNote {
                pubkey: "a".repeat(64),
                created_at: 1_700_000_000,
                kind: 1,
                tags,
                content: "hello 🐾".to_string(),
                id: Some("b".repeat(64)),
                sig: Some("c".repeat(128)),
            }
        }

        fn outbound() -> Vec<Outbound> {
            vec![
                Outbound::Note(Self::note()),
                Outbound::RelayEvent(r#"["EOSE","sub"]"#.to_string()),
                Outbound::RelayHealth(vec![
                    ("wss://relay.one".to_string(), 1),
                    ("wss://relay.two".to_string(), 3),
                ]),
                Outbound::RelayHealth(Vec::new()),
            ]
        }

        fn inbound() -> Vec<Inbound> {
            vec![
                Inbound::Connect(vec!["wss://a.io".to_string(), "wss://b.io".to_string()]),
                Inbound::Connect(Vec::new()),
                Inbound::AddRelay("wss://c.io".to_string()),
                Inbound::RemoveRelay("wss://c.io".to_string()),
                Inbound::Subscribe {
                    sub_id: "sub-1".to_string(),
                    filter_json: r#"{"kinds":[1]}"#.to_string(),
                    req_json: r#"["REQ","sub-1",{"kinds":[1]}]"#.to_string(),
                },
                Inbound::Close("sub-1".to_string()),
                Inbound::Send(r#"["EVENT",{}]"#.to_string()),
            ]
        }
    }

    #[test]
    fn every_outbound_variant_round_trips() {
        for message in Sample::outbound() {
            let frame = Envelope::encode_outbound(&message);
            assert_eq!(Envelope::decode_outbound(&frame), Some(message));
        }
    }

    #[test]
    fn every_inbound_variant_round_trips() {
        for message in Sample::inbound() {
            let frame = Envelope::encode_inbound(&message);
            assert_eq!(Envelope::decode_inbound(&frame), Some(message));
        }
    }

    #[test]
    fn the_two_directions_share_no_tag() {
        let unique = |mut tags: Vec<u8>| {
            tags.sort_unstable();
            tags.dedup();
            tags
        };
        let out = unique(
            Sample::outbound()
                .iter()
                .map(|m| Envelope::encode_outbound(m)[0])
                .collect(),
        );
        let inb = unique(
            Sample::inbound()
                .iter()
                .map(|m| Envelope::encode_inbound(m)[0])
                .collect(),
        );
        assert_eq!(out.len(), 3, "three outbound variants");
        assert_eq!(inb.len(), 6, "six inbound variants");
        assert!(
            out.iter().all(|tag| !inb.contains(tag)),
            "a tag reused across directions would let a frame decode as the wrong message"
        );
    }

    #[test]
    fn a_direction_never_decodes_as_the_other() {
        for message in Sample::inbound() {
            let frame = Envelope::encode_inbound(&message);
            assert_eq!(
                Envelope::decode_outbound(&frame),
                None,
                "inbound frame must not decode as outbound"
            );
        }
        for message in Sample::outbound() {
            let frame = Envelope::encode_outbound(&message);
            assert_eq!(Envelope::decode_inbound(&frame), None);
        }
    }

    #[test]
    fn truncation_is_rejected_at_every_cut() {
        for message in Sample::outbound() {
            let frame = Envelope::encode_outbound(&message);
            for cut in 0..frame.len() {
                assert_eq!(Envelope::decode_outbound(&frame[..cut]), None, "cut {cut}");
            }
        }
        for message in Sample::inbound() {
            let frame = Envelope::encode_inbound(&message);
            for cut in 0..frame.len() {
                assert_eq!(Envelope::decode_inbound(&frame[..cut]), None, "cut {cut}");
            }
        }
    }

    #[test]
    fn trailing_garbage_is_rejected() {
        for message in Sample::outbound() {
            let mut frame = Envelope::encode_outbound(&message);
            frame.push(0);
            assert_eq!(Envelope::decode_outbound(&frame), None);
        }
        for message in Sample::inbound() {
            let mut frame = Envelope::encode_inbound(&message);
            frame.push(0);
            assert_eq!(Envelope::decode_inbound(&frame), None);
        }
    }

    #[test]
    fn unknown_tag_is_rejected() {
        assert_eq!(Envelope::decode_outbound(&[200, 0, 0, 0, 0]), None);
        assert_eq!(Envelope::decode_inbound(&[200, 0, 0, 0, 0]), None);
    }

    #[test]
    fn empty_frame_is_rejected() {
        assert_eq!(Envelope::decode_outbound(&[]), None);
        assert_eq!(Envelope::decode_inbound(&[]), None);
    }

    #[test]
    fn invalid_utf8_is_rejected_without_panicking() {
        let mut frame = Envelope::encode_outbound(&Outbound::RelayEvent("ascii".to_string()));
        let at = frame
            .windows(5)
            .position(|w| w == b"ascii")
            .expect("payload present");
        frame[at] = 0xff;
        assert_eq!(Envelope::decode_outbound(&frame), None);
    }

    #[test]
    fn corrupt_length_prefix_is_rejected() {
        let mut frame = Envelope::encode_inbound(&Inbound::Close("sub-1".to_string()));
        frame[1..5].copy_from_slice(&1_000_000_u32.to_le_bytes());
        assert_eq!(Envelope::decode_inbound(&frame), None);
    }
}

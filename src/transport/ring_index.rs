//! Pure index arithmetic for the shared byte ring.
//!
//! Split from the ring itself because this is the part where an off-by-one
//! hides, and it is the part that needs no `SharedArrayBuffer` — so it builds
//! and tests on the host, with no browser.

/// Where a frame goes, once the wrap has been accounted for.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Reservation {
    /// Byte offset of the skip marker, when the frame does not fit in the
    /// contiguous run at the end of the region.
    pub skip_at: Option<u32>,
    /// Byte offset of the frame's length word.
    pub at: u32,
    /// How far the tail advances, including any skipped tail bytes.
    pub advance: u32,
}

/// Head/tail arithmetic over a power-of-two byte region.
#[derive(Clone, Copy)]
pub struct RingIndex {
    cap: u32,
    mask: u32,
}

impl RingIndex {
    /// # Panics
    /// Panics if `cap` is not a power of two, or is smaller than one word.
    #[must_use]
    pub const fn new(cap: u32) -> Self {
        assert!(cap.is_power_of_two(), "ring capacity must be a power of two");
        assert!(cap >= 4, "ring capacity must hold at least one length word");
        Self {
            cap,
            mask: cap - 1,
        }
    }

    /// Total bytes a frame occupies: a 4-byte length word plus the payload,
    /// rounded up so the next frame stays 4-byte aligned.
    #[must_use]
    pub const fn frame_bytes(self, len: u32) -> u32 {
        (4 + len).div_ceil(4) * 4
    }

    #[must_use]
    pub const fn offset(self, cursor: i32) -> u32 {
        cursor.cast_unsigned() & self.mask
    }

    #[must_use]
    pub const fn used(self, head: i32, tail: i32) -> u32 {
        tail.wrapping_sub(head).cast_unsigned()
    }

    /// Bytes from `cursor` to the end of the region.
    #[must_use]
    pub const fn contiguous(self, cursor: i32) -> u32 {
        self.cap - self.offset(cursor)
    }

    /// Plan a write of `len` payload bytes, or `None` if it does not fit.
    #[must_use]
    pub fn reserve(self, head: i32, tail: i32, len: u32) -> Option<Reservation> {
        let need = self.frame_bytes(len);
        if need > self.cap {
            return None;
        }
        let start = self.offset(tail);
        let contiguous = self.cap - start;
        let skip = if contiguous < need { contiguous } else { 0 };
        let free = self.cap - self.used(head, tail);
        if free < skip + need {
            return None;
        }
        Some(Reservation {
            skip_at: (skip > 0).then_some(start),
            at: if skip > 0 { 0 } else { start },
            advance: skip + need,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Reservation, RingIndex};

    const CAP: u32 = 64;

    fn index() -> RingIndex {
        RingIndex::new(CAP)
    }

    #[test]
    fn frame_bytes_pads_to_word_boundary() {
        let ix = index();
        assert_eq!(ix.frame_bytes(0), 4);
        assert_eq!(ix.frame_bytes(1), 8);
        assert_eq!(ix.frame_bytes(3), 8);
        assert_eq!(ix.frame_bytes(4), 8);
        assert_eq!(ix.frame_bytes(5), 12);
    }

    #[test]
    fn reserve_from_empty_starts_at_zero() {
        let ix = index();
        assert_eq!(
            ix.reserve(0, 0, 4),
            Some(Reservation {
                skip_at: None,
                at: 0,
                advance: 8,
            })
        );
    }

    #[test]
    fn reserve_rejects_frame_larger_than_region() {
        let ix = index();
        assert_eq!(ix.reserve(0, 0, CAP), None);
    }

    #[test]
    fn largest_fitting_frame_is_accepted() {
        let ix = index();
        let reservation = ix.reserve(0, 0, CAP - 4).expect("largest frame must fit");
        assert_eq!(reservation.advance, CAP);
    }

    #[test]
    fn reserve_skips_a_short_tail_run() {
        let ix = index();
        let head = (CAP / 2) as i32;
        let tail = (CAP - 8) as i32;
        let reservation = ix.reserve(head, tail, 8).expect("should wrap");
        assert_eq!(reservation.skip_at, Some(CAP - 8));
        assert_eq!(reservation.at, 0);
        assert_eq!(reservation.advance, 8 + 12);
    }

    #[test]
    fn reserve_refuses_to_wrap_when_the_skip_would_pass_the_head() {
        let ix = index();
        let tail = (CAP - 8) as i32;
        assert_eq!(ix.reserve(0, tail, 8), None);
    }

    #[test]
    fn reserve_counts_skipped_bytes_against_free_space() {
        let ix = index();
        let head = (CAP - 8) as i32;
        let tail = (2 * CAP - 8) as i32;
        assert_eq!(ix.used(head, tail), CAP);
        assert_eq!(ix.reserve(head, tail, 0), None);
    }

    #[test]
    fn reserve_fails_when_full() {
        let ix = index();
        assert_eq!(ix.reserve(0, CAP as i32, 0), None);
    }

    #[test]
    fn cursors_survive_wrapping_past_i32_max() {
        let ix = index();
        let head = i32::MAX - 3;
        let tail = head.wrapping_add(8);
        assert_eq!(ix.used(head, tail), 8);
        assert!(ix.reserve(head, tail, 4).is_some());
    }

    #[test]
    fn drain_and_refill_returns_every_offset_in_range() {
        let ix = index();
        let mut head = 0i32;
        let mut tail = 0i32;
        for _ in 0..256 {
            let Some(reservation) = ix.reserve(head, tail, 5) else {
                head = tail;
                continue;
            };
            assert!(reservation.at + ix.frame_bytes(5) <= CAP);
            tail = tail.wrapping_add(reservation.advance as i32);
            assert!(ix.used(head, tail) <= CAP);
        }
    }
}

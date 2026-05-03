//! A bounded, const-generic ring buffer for "recent N events".
//!
//! The substrate keeps several "recent" rings: recent gaze fixations,
//! recent committed directives, recent active windows. They share one
//! invariant: **they never grow unbounded**, no matter how busy the user
//! gets. A const-generic capacity makes the bound a compile-time fact.
//!
//! ## Why const generic
//!
//! - **The size is part of the type.** A `RingBuffer<GazeFixation, 64>` and
//!   a `RingBuffer<GazeFixation, 128>` are *different types*. Mixing them
//!   up at a join is a compile error, not a runtime accident.
//! - **No allocation on the hot path.** The buffer is a stack array
//!   (`[Option<T>; N]`), so push is `O(1)` with zero allocations.
//! - **Capacity zero is rejected at compile time.** We assert
//!   `N > 0` via a const-eval check.
//!
//! ## Iteration order
//!
//! [`RingBuffer::iter`] yields oldest-first. [`RingBuffer::iter_recent`]
//! yields newest-first. The substrate uses both — newest-first for "what
//! was the user just looking at?", oldest-first for "what's the trajectory?".

use std::array;

use serde::de::{Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

/// A fixed-capacity FIFO ring. Pushing into a full ring overwrites the
/// oldest entry.
///
/// `N` is a const generic: the capacity is encoded in the type. `N == 0` is
/// rejected at instantiation by a const-eval assertion.
///
/// ## Invariants enforced
///
/// 1. `len() <= N` always.
/// 2. After exactly `N` pushes the buffer is full; subsequent pushes
///    overwrite oldest.
/// 3. `iter()` yields exactly `len()` items, oldest-first.
/// 4. `iter_recent()` yields exactly `len()` items, newest-first.
/// 5. `most_recent()` is the most recently pushed item, or `None` if empty.
///
/// ## Serialization
///
/// Serializes as a flat sequence of `T` in oldest-first iteration order
/// (the same shape as [`RingBuffer::iter`] yields). The internal
/// representation (`buf`, `head`, `len`) is *not* part of the wire
/// format — that lets us refactor storage without breaking journals.
/// On deserialize, sequences longer than `N` cause the leading items to
/// be evicted, matching `push` semantics.
#[derive(Debug, Clone)]
pub struct RingBuffer<T, const N: usize> {
    /// Storage. `None` slots are unfilled; `Some` slots contain values.
    /// We use `Option<T>` rather than `MaybeUninit<T>` to avoid `unsafe`.
    buf: [Option<T>; N],
    /// Index of the next write position, `0..N`.
    head: usize,
    /// Number of currently-filled slots, `0..=N`.
    len: usize,
}

impl<T, const N: usize> RingBuffer<T, N> {
    /// Compile-time assertion that `N > 0`. A zero-capacity ring is a
    /// usage bug; we'd rather panic in monomorphization than silently
    /// produce a no-op buffer.
    const NON_ZERO_CAPACITY: () = assert!(N > 0, "RingBuffer capacity must be > 0");

    /// Construct an empty ring of capacity `N`.
    #[must_use]
    pub fn new() -> Self {
        // Force the const-eval check.
        let () = Self::NON_ZERO_CAPACITY;
        Self {
            buf: array::from_fn(|_| None),
            head: 0,
            len: 0,
        }
    }

    /// Capacity (the const generic `N`).
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of items currently stored, `0..=N`.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// `true` when no items have been pushed yet.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `true` when the next push will overwrite the oldest item.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Push an item. If the ring is full, the oldest item is dropped and
    /// returned. Otherwise returns `None`.
    pub fn push(&mut self, item: T) -> Option<T> {
        let evicted = self.buf[self.head].take();
        self.buf[self.head] = Some(item);
        self.head = (self.head + 1) % N;
        if self.len < N {
            self.len += 1;
            // First N pushes evict nothing.
            None
        } else {
            // Subsequent pushes evict the oldest.
            evicted
        }
    }

    /// Most recently pushed item, or `None` if empty.
    #[must_use]
    pub fn most_recent(&self) -> Option<&T> {
        if self.is_empty() {
            return None;
        }
        // Newest is at head - 1 (modulo N).
        let idx = (self.head + N - 1) % N;
        self.buf[idx].as_ref()
    }

    /// Oldest-first iterator over filled slots.
    ///
    /// When the ring is not full, items live in `[0..len)` in insertion
    /// order. When full, the oldest is at `head`. We handle both with a
    /// single state machine.
    pub fn iter(&self) -> RingIter<'_, T, N> {
        let start = if self.is_full() { self.head } else { 0 };
        RingIter {
            buf: &self.buf,
            cursor: start,
            remaining: self.len,
        }
    }

    /// Newest-first iterator. Reverse of [`RingBuffer::iter`].
    pub fn iter_recent(&self) -> RingIterRev<'_, T, N> {
        let end = (self.head + N - 1) % N;
        RingIterRev {
            buf: &self.buf,
            cursor: end,
            remaining: self.len,
        }
    }

    /// Drop all items.
    pub fn clear(&mut self) {
        for slot in &mut self.buf {
            *slot = None;
        }
        self.head = 0;
        self.len = 0;
    }
}

impl<T, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PartialEq, const N: usize> PartialEq for RingBuffer<T, N> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        self.iter().eq(other.iter())
    }
}

// --- Serde manual impls ------------------------------------------------------
//
// We can't derive `Serialize`/`Deserialize` because serde does not provide a
// generic `[T; N]: Serialize` for arbitrary const `N`. We also don't want the
// internal `(buf, head, len)` shape on the wire. So we serialize as a flat
// sequence of items in oldest-first order, and deserialize by pushing into
// a fresh ring (overwriting if the input exceeds capacity, just like `push`).

impl<T: Serialize, const N: usize> Serialize for RingBuffer<T, N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for item in self {
            seq.serialize_element(item)?;
        }
        seq.end()
    }
}

impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for RingBuffer<T, N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RingVisitor<T, const N: usize>(std::marker::PhantomData<T>);

        impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for RingVisitor<T, N> {
            type Value = RingBuffer<T, N>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "a sequence of items, oldest-first")
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut ring = RingBuffer::<T, N>::new();
                while let Some(item) = seq.next_element::<T>()? {
                    ring.push(item);
                }
                Ok(ring)
            }
        }

        deserializer.deserialize_seq(RingVisitor::<T, N>(std::marker::PhantomData))
    }
}

// --- IntoIterator impls ------------------------------------------------------
//
// `&RingBuffer<T, N>` and `&mut RingBuffer<T, N>` both implement `IntoIterator`
// so they fit naturally into `for` loops without going through `.iter()`.
// The default direction is oldest-first, matching `iter()`.

impl<'a, T, const N: usize> IntoIterator for &'a RingBuffer<T, N> {
    type Item = &'a T;
    type IntoIter = RingIter<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// --- Iterators ---------------------------------------------------------------

/// Oldest-first iterator over a [`RingBuffer`].
pub struct RingIter<'a, T, const N: usize> {
    buf: &'a [Option<T>; N],
    cursor: usize,
    remaining: usize,
}

impl<'a, T, const N: usize> Iterator for RingIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        // The ring is built so we always advance to a `Some` slot; assert
        // it as a safety net rather than panicking with an opaque message.
        let item = self.buf[self.cursor]
            .as_ref()
            .expect("ring buffer cursor on empty slot — invariant violated");
        self.cursor = (self.cursor + 1) % N;
        self.remaining -= 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, const N: usize> ExactSizeIterator for RingIter<'_, T, N> {}

/// Newest-first iterator over a [`RingBuffer`].
pub struct RingIterRev<'a, T, const N: usize> {
    buf: &'a [Option<T>; N],
    cursor: usize,
    remaining: usize,
}

impl<'a, T, const N: usize> Iterator for RingIterRev<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let item = self.buf[self.cursor]
            .as_ref()
            .expect("ring buffer cursor on empty slot — invariant violated");
        self.cursor = (self.cursor + N - 1) % N;
        self.remaining -= 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T, const N: usize> ExactSizeIterator for RingIterRev<'_, T, N> {}

// --- Module tests ------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_ring_has_no_items() {
        let ring: RingBuffer<u32, 4> = RingBuffer::new();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert_eq!(ring.capacity(), 4);
        assert_eq!(ring.most_recent(), None);
        assert_eq!(ring.iter().count(), 0);
        assert_eq!(ring.iter_recent().count(), 0);
    }

    #[test]
    fn push_under_capacity_does_not_evict() {
        let mut ring: RingBuffer<u32, 4> = RingBuffer::new();
        assert_eq!(ring.push(1), None);
        assert_eq!(ring.push(2), None);
        assert_eq!(ring.push(3), None);
        assert_eq!(ring.len(), 3);
        assert!(!ring.is_full());
        assert_eq!(ring.most_recent(), Some(&3));
    }

    #[test]
    fn push_over_capacity_evicts_oldest() {
        let mut ring: RingBuffer<u32, 3> = RingBuffer::new();
        for n in 1..=3 {
            ring.push(n);
        }
        assert!(ring.is_full());
        // 4th push evicts the oldest (1):
        assert_eq!(ring.push(4), Some(1));
        // 5th push evicts 2:
        assert_eq!(ring.push(5), Some(2));
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.most_recent(), Some(&5));
    }

    #[test]
    fn iter_yields_oldest_first() {
        let mut ring: RingBuffer<u32, 3> = RingBuffer::new();
        for n in 1..=5 {
            ring.push(n); // After loop the ring contains [3, 4, 5].
        }
        let collected: Vec<_> = ring.iter().copied().collect();
        assert_eq!(collected, vec![3, 4, 5]);
    }

    #[test]
    fn iter_recent_yields_newest_first() {
        let mut ring: RingBuffer<u32, 3> = RingBuffer::new();
        for n in 1..=5 {
            ring.push(n);
        }
        let collected: Vec<_> = ring.iter_recent().copied().collect();
        assert_eq!(collected, vec![5, 4, 3]);
    }

    #[test]
    fn clear_resets() {
        let mut ring: RingBuffer<u32, 3> = RingBuffer::new();
        ring.push(1);
        ring.push(2);
        ring.clear();
        assert_eq!(ring.len(), 0);
        assert!(ring.is_empty());
        assert_eq!(ring.most_recent(), None);
    }
}

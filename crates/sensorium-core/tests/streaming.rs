//! Tests for the generation-tagged streaming substrate.
//!
//! The substrate is the contract that lets every voice/intent/agent
//! stage in the MIL pipeline emit and consume *speculative*, *partial*,
//! and *finalized* values along a stream — with generation numbers
//! that make cancellation safe.
//!
//! Properties covered:
//!
//! 1. **Generation is monotonic, copy-cheap, and serializes transparently** —
//!    a fresh `GenerationSeq` issues `INITIAL`, then strictly-increasing
//!    counters; serializing a `Generation` round-trips through JSON as
//!    a bare `u64`.
//! 2. **GenerationSeq is thread-safe** — concurrent `advance()` calls
//!    issue distinct values; spawning N threads each minting M
//!    generations produces N×M unique generations.
//! 3. **StreamUpdate carries generation in every variant** — `Partial`,
//!    `Final`, and `Cancelled` all expose `.generation()` without
//!    pattern-match juggling.
//! 4. **`value()` and `into_value()` return `None` only for `Cancelled`** —
//!    callers can extract the value variantly without a discriminant.
//! 5. **`is_partial / is_final / is_cancelled` are exclusive** — exactly
//!    one returns `true` for any given update.
//! 6. **`map` is a functor** — `update.map(f).map(g) == update.map(|x| g(f(x)))`,
//!    and `Cancelled` stays `Cancelled` through any map.
//! 7. **Serde round-trip is value-preserving** — `Partial`, `Final`,
//!    `Cancelled` all round-trip through JSON identically.

use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

use sensorium_core::{Generation, GenerationSeq, StreamUpdate};

// --- Property 1: Generation basics -----------------------------------------

#[test]
fn fresh_seq_starts_at_initial_then_advances_monotonically() {
    let seq = GenerationSeq::new();
    assert_eq!(seq.current(), Generation::INITIAL);
    let g0 = seq.advance();
    let g1 = seq.advance();
    let g2 = seq.advance();
    assert_eq!(g0, Generation::INITIAL);
    assert!(g1 > g0);
    assert!(g2 > g1);
}

#[test]
fn generation_is_copy_and_eq_hash() {
    let g = Generation::new(42);
    let copy = g;
    assert_eq!(g, copy);
    let mut set = HashSet::new();
    set.insert(g);
    assert!(set.contains(&copy));
}

#[test]
fn generation_into_inner_round_trips() {
    let g = Generation::new(12_345);
    assert_eq!(g.into_inner(), 12_345);
}

#[test]
fn generation_serializes_transparently_as_u64() {
    let g = Generation::new(7);
    let json = serde_json::to_string(&g).expect("serialize");
    assert_eq!(json, "7");
    let back: Generation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, g);
}

#[test]
fn generation_seq_current_does_not_advance() {
    let seq = GenerationSeq::new();
    let _ = seq.current();
    let _ = seq.current();
    assert_eq!(seq.current(), Generation::INITIAL);
    let g0 = seq.advance();
    assert_eq!(g0, Generation::INITIAL);
    assert_eq!(seq.current(), Generation::new(1));
}

#[test]
fn generation_seq_default_matches_new() {
    let a = GenerationSeq::new();
    let b = GenerationSeq::default();
    assert_eq!(a.current(), b.current());
}

// --- Property 2: Thread safety ---------------------------------------------

#[test]
fn concurrent_advance_yields_distinct_generations() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 1000;
    let seq = Arc::new(GenerationSeq::new());

    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let seq = Arc::clone(&seq);
            thread::spawn(move || {
                let mut local = Vec::with_capacity(PER_THREAD);
                for _ in 0..PER_THREAD {
                    local.push(seq.advance());
                }
                local
            })
        })
        .collect();

    let mut all = HashSet::with_capacity(THREADS * PER_THREAD);
    for h in handles {
        for g in h.join().expect("thread") {
            assert!(all.insert(g), "duplicate generation: {g:?}");
        }
    }
    assert_eq!(all.len(), THREADS * PER_THREAD);
}

// --- Property 3: StreamUpdate carries generation ---------------------------

#[test]
fn stream_update_partial_exposes_generation() {
    let g = Generation::new(5);
    let u = StreamUpdate::Partial {
        generation: g,
        value: "hello",
    };
    assert_eq!(u.generation(), g);
}

#[test]
fn stream_update_final_exposes_generation() {
    let g = Generation::new(10);
    let u = StreamUpdate::Final {
        generation: g,
        value: 42_u32,
    };
    assert_eq!(u.generation(), g);
}

#[test]
fn stream_update_cancelled_exposes_generation() {
    let g = Generation::new(99);
    let u: StreamUpdate<String> = StreamUpdate::Cancelled { generation: g };
    assert_eq!(u.generation(), g);
}

// --- Property 4: value extraction ------------------------------------------

#[test]
fn partial_value_is_some() {
    let u = StreamUpdate::Partial {
        generation: Generation::INITIAL,
        value: "hi",
    };
    assert_eq!(u.value(), Some(&"hi"));
    assert_eq!(u.into_value(), Some("hi"));
}

#[test]
fn final_value_is_some() {
    let u = StreamUpdate::Final {
        generation: Generation::INITIAL,
        value: 7_u32,
    };
    assert_eq!(u.value(), Some(&7));
    assert_eq!(u.into_value(), Some(7));
}

#[test]
fn cancelled_value_is_none() {
    let u: StreamUpdate<String> = StreamUpdate::Cancelled {
        generation: Generation::INITIAL,
    };
    assert!(u.value().is_none());
    assert!(u.into_value().is_none());
}

// --- Property 5: variant predicates are exclusive --------------------------

#[test]
fn is_partial_only_for_partial() {
    let p = StreamUpdate::Partial {
        generation: Generation::INITIAL,
        value: 1,
    };
    let f = StreamUpdate::Final {
        generation: Generation::INITIAL,
        value: 1,
    };
    let c: StreamUpdate<i32> = StreamUpdate::Cancelled {
        generation: Generation::INITIAL,
    };

    assert!(p.is_partial() && !p.is_final() && !p.is_cancelled());
    assert!(!f.is_partial() && f.is_final() && !f.is_cancelled());
    assert!(!c.is_partial() && !c.is_final() && c.is_cancelled());
}

// --- Property 6: map is a functor ------------------------------------------

#[test]
fn map_partial_transforms_value_preserves_generation() {
    let g = Generation::new(3);
    let u = StreamUpdate::Partial {
        generation: g,
        value: 4_i32,
    };
    let mapped = u.map(|n| n * 2);
    match mapped {
        StreamUpdate::Partial { generation, value } => {
            assert_eq!(generation, g);
            assert_eq!(value, 8);
        }
        _ => panic!("expected Partial after map"),
    }
}

#[test]
fn map_final_transforms_value_preserves_generation() {
    let g = Generation::new(11);
    let u = StreamUpdate::Final {
        generation: g,
        value: "abc",
    };
    let mapped = u.map(str::len);
    match mapped {
        StreamUpdate::Final { generation, value } => {
            assert_eq!(generation, g);
            assert_eq!(value, 3);
        }
        _ => panic!("expected Final after map"),
    }
}

#[test]
fn map_cancelled_stays_cancelled() {
    let g = Generation::new(50);
    let u: StreamUpdate<i32> = StreamUpdate::Cancelled { generation: g };
    let mapped = u.map(|n| n.to_string());
    match mapped {
        StreamUpdate::Cancelled { generation } => assert_eq!(generation, g),
        _ => panic!("expected Cancelled after map"),
    }
}

#[test]
fn map_composes() {
    let g = Generation::new(7);
    let u = StreamUpdate::Partial {
        generation: g,
        value: 5_i32,
    };
    let a = u.clone().map(|n| n + 1).map(|n| n * 3);
    let b = u.map(|n| (n + 1) * 3);
    assert_eq!(a, b);
}

// --- Property 7: serde round-trip ------------------------------------------

#[test]
fn partial_round_trips_through_json() {
    let u = StreamUpdate::Partial {
        generation: Generation::new(2),
        value: "hello".to_owned(),
    };
    let json = serde_json::to_string(&u).expect("serialize");
    let back: StreamUpdate<String> = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(u, back);
}

#[test]
fn final_round_trips_through_json() {
    let u = StreamUpdate::Final {
        generation: Generation::new(2),
        value: 42_u32,
    };
    let json = serde_json::to_string(&u).expect("serialize");
    let back: StreamUpdate<u32> = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(u, back);
}

#[test]
fn cancelled_round_trips_through_json() {
    let u: StreamUpdate<String> = StreamUpdate::Cancelled {
        generation: Generation::new(99),
    };
    let json = serde_json::to_string(&u).expect("serialize");
    let back: StreamUpdate<String> = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(u, back);
}

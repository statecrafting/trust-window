// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Bartek Kus
//
// Relicensed from the Open Agentic Platform (crates/policy-kernel/coherence.rs,
// AGPL-3.0-or-later) to Apache-2.0 by the sole copyright holder. See NOTICE.

//! `trust-window`: a rolling-window trust scorer with a graduated privilege
//! output.
//!
//! Feed a stream of [`Sample`]s (graded, weighted observations in `[0, 1]`),
//! read back a decay-weighted [`score`](WindowScorer::score) and a
//! [`Level`]. The score-to-level mapping direction is configuration:
//!
//! - [`Direction::DegradeOnly`] latches to the worst level reached and only
//!   recovers on an explicit [`restore_max`](WindowScorer::restore_max). This
//!   is OAP's original `CoherenceScheduler` behavior: an agent's privilege
//!   ratchets down within a run and a human clears it.
//! - [`Direction::Bidirectional`] tracks the score both ways: the level
//!   promotes as trust recovers and demotes as it regresses, with promotion to
//!   [`Level::Full`] gated on a minimum number of observations. This is what a
//!   cross-session autonomy ladder needs.
//!
//! # What this crate is and is not
//!
//! It ships the **scorer**: the pure, deterministic score-to-level primitive,
//! plus a [snapshot](WindowSnapshot) seam so a consumer can persist a
//! per-subject window (per agent, per play, per whatever) to durable storage
//! and rehydrate it. It does **not** ship the ladder: which subject a window
//! belongs to, where snapshots live, when a promotion becomes an autonomy
//! change, or which signals feed the samples are all the consumer's concern.
//!
//! # Determinism
//!
//! Given the same sample sequence and config, [`score`](WindowScorer::score)
//! and [`level`](WindowScorer::level) are reproducible: there is no wall clock
//! and no hidden state. The window ages samples by insertion order, not by time.
//!
//! ```
//! use trust_window::{Direction, Level, Sample, WindowConfig, WindowScorer};
//!
//! let mut cfg = WindowConfig::default();
//! cfg.direction = Direction::Bidirectional;
//! let mut scorer = WindowScorer::new(cfg);
//! for _ in 0..50 {
//!     scorer.record(Sample::aligned(true));
//! }
//! assert_eq!(scorer.level(), Level::Full);
//! ```

mod config;
mod scorer;

pub use config::{Direction, Level, LevelThresholds, Sample, WindowConfig, WindowSnapshot};
pub use scorer::WindowScorer;

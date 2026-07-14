//! Configuration and value types for the trust window.

use serde::{Deserialize, Serialize};

/// One observation fed into the window.
///
/// `value` is a graded signal in `[0, 1]` (1.0 = fully aligned / accepted,
/// 0.0 = fully violating / rejected). `weight` scales the sample's influence
/// (default 1.0). OAP's boolean `aligned` is the special case
/// `Sample { value: 1.0 | 0.0, weight: 1.0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub value: f64,
    pub weight: f64,
}

impl Sample {
    /// A unit-weight sample with the given value (clamped to `[0, 1]`).
    pub fn new(value: f64) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            weight: 1.0,
        }
    }

    /// A weighted sample (both value and weight clamped to sane ranges:
    /// value to `[0, 1]`, weight to `>= 0`).
    pub fn weighted(value: f64, weight: f64) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            weight: weight.max(0.0),
        }
    }

    /// The boolean special case: `true` -> 1.0, `false` -> 0.0, weight 1.0.
    /// This is exactly OAP's `record_action(aligned)` input.
    pub fn aligned(aligned: bool) -> Self {
        Self {
            value: if aligned { 1.0 } else { 0.0 },
            weight: 1.0,
        }
    }
}

/// Graduated privilege level derived from the window score. Higher variants are
/// more permissive; `severity()` orders them the other way (higher = worse).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Score at or above the `full` threshold. All operations available.
    Full,
    /// Between `restricted` and `full`. A middle tier.
    Restricted,
    /// Between `read_only` and `restricted`. Low trust.
    ReadOnly,
    /// Below the `read_only` threshold. Lowest trust.
    Suspended,
}

impl Level {
    /// Higher = less permissive (worse). Mirrors OAP's `PrivilegeLevel::severity`.
    #[inline]
    pub fn severity(self) -> u8 {
        match self {
            Level::Full => 0,
            Level::Restricted => 1,
            Level::ReadOnly => 2,
            Level::Suspended => 3,
        }
    }

    /// The worst (most restrictive) of two levels.
    #[inline]
    pub fn max_severity(a: Self, b: Self) -> Self {
        if a.severity() >= b.severity() { a } else { b }
    }

    /// Reconstruct a level from a severity value (0..=3), saturating.
    #[inline]
    pub fn from_severity(s: u8) -> Self {
        match s {
            0 => Level::Full,
            1 => Level::Restricted,
            2 => Level::ReadOnly,
            _ => Level::Suspended,
        }
    }
}

/// Score cutoffs for each level. A score `s` maps to `Full` when
/// `s >= full`, `Restricted` when `s >= restricted`, `ReadOnly` when
/// `s >= read_only`, else `Suspended`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LevelThresholds {
    pub full: f64,
    pub restricted: f64,
    pub read_only: f64,
}

impl Default for LevelThresholds {
    /// OAP's bands: Full >= 0.8, Restricted >= 0.5, ReadOnly >= 0.2.
    fn default() -> Self {
        Self {
            full: 0.8,
            restricted: 0.5,
            read_only: 0.2,
        }
    }
}

impl LevelThresholds {
    /// The level implied by `score` under these thresholds (before any
    /// violation floor or degrade-only latch).
    pub fn level_of(&self, score: f64) -> Level {
        if score >= self.full {
            Level::Full
        } else if score >= self.restricted {
            Level::Restricted
        } else if score >= self.read_only {
            Level::ReadOnly
        } else {
            Level::Suspended
        }
    }
}

/// How the reported level is allowed to move over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    /// Monotonic down within a run: the level ratchets to its worst and stays
    /// there until [`WindowScorer::restore_max`] clears the latch. This is
    /// OAP's `CoherenceScheduler` behavior (SC-008).
    ///
    /// [`WindowScorer::restore_max`]: crate::WindowScorer::restore_max
    DegradeOnly,
    /// The level tracks the score both ways: it promotes when the score
    /// recovers and demotes when it regresses. Promotion to `Full` requires at
    /// least `promote_min_samples` observations in the window.
    Bidirectional,
}

/// Tunables for a [`WindowScorer`](crate::WindowScorer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Rolling window length (default 50).
    pub window_size: usize,
    /// How the reported level may move (default `DegradeOnly`, matching OAP).
    pub direction: Direction,
    /// Newest sample weight is 1.0; each step older multiplies by this decay
    /// (default 0.95). Clamped to `[0, 1]` at score time.
    pub decay_lambda: f64,
    /// Score cutoffs per level.
    pub thresholds: LevelThresholds,
    /// A sample counts as a violation when its `value` is strictly below this
    /// threshold (default 0.5). OAP's boolean `false` (value 0.0) is a
    /// violation; `true` (value 1.0) is not.
    pub violation_threshold: f64,
    /// When the violation count in the window reaches this, the level is at
    /// least `Restricted` (OAP's SC-007 floor; default 3). Set very high to
    /// disable.
    pub violation_floor: u32,
    /// `Bidirectional` only: promotion to `Full` requires at least this many
    /// samples in the window (default 0 = no gating). Ignored in `DegradeOnly`.
    pub promote_min_samples: usize,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_size: 50,
            direction: Direction::DegradeOnly,
            decay_lambda: 0.95,
            thresholds: LevelThresholds::default(),
            violation_threshold: 0.5,
            violation_floor: 3,
            promote_min_samples: 0,
        }
    }
}

/// A serializable snapshot of a scorer's mutable state, for durable persistence
/// and rehydration. The window contents plus the degrade-only latch are all
/// that is needed to resume scoring identically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WindowSnapshot {
    pub samples: Vec<Sample>,
    /// The degrade-only latch (worst severity reached). Zero for a
    /// bidirectional scorer or a fresh window.
    #[serde(default)]
    pub stuck_severity: u8,
}

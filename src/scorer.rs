//! The rolling-window scorer.

use std::collections::VecDeque;

use crate::config::{Direction, Level, Sample, WindowConfig, WindowSnapshot};

/// A rolling-window trust scorer.
///
/// Feeds a fixed-size window of [`Sample`]s, computes a decay-weighted score in
/// `[0, 1]`, and maps that to a graduated [`Level`]. The mapping direction is
/// configurable: `DegradeOnly` latches to the worst level reached (OAP's
/// original behavior), `Bidirectional` tracks the score both ways.
///
/// Deterministic: given the same sample sequence and config, `score()` and
/// `level()` are reproducible (no wall clock). State is
/// [snapshot](WindowScorer::snapshot)-serializable so a consumer can persist a
/// per-subject window and rehydrate it later without the library knowing about
/// any storage.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowScorer {
    config: WindowConfig,
    window: VecDeque<Sample>,
    /// Worst severity reached without a restore (the `DegradeOnly` latch).
    stuck_severity: u8,
}

impl WindowScorer {
    /// A fresh scorer with the given config.
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            window: VecDeque::new(),
            stuck_severity: 0,
        }
    }

    /// A fresh scorer with default config (window 50, `DegradeOnly`, OAP bands).
    pub fn with_defaults() -> Self {
        Self::new(WindowConfig::default())
    }

    /// Rehydrate a scorer from a persisted [`WindowSnapshot`].
    pub fn from_snapshot(config: WindowConfig, snapshot: WindowSnapshot) -> Self {
        Self {
            config,
            window: snapshot.samples.into_iter().collect(),
            stuck_severity: snapshot.stuck_severity,
        }
    }

    /// The config this scorer was built with.
    pub fn config(&self) -> &WindowConfig {
        &self.config
    }

    /// Record one sample, evicting the oldest when the window is full.
    pub fn record(&mut self, sample: Sample) {
        if self.window.len() >= self.config.window_size {
            self.window.pop_front();
        }
        self.window.push_back(sample);
        self.update_stuck();
    }

    /// Convenience for the boolean signal source: `record(Sample::aligned(b))`.
    /// This is exactly OAP's `record_action(aligned)`.
    pub fn record_aligned(&mut self, aligned: bool) {
        self.record(Sample::aligned(aligned));
    }

    /// The decay-weighted score in `[0, 1]`. An empty window scores 1.0 (maximum
    /// trust: nothing has gone wrong yet), matching OAP.
    pub fn score(&self) -> f64 {
        let n = self.window.len();
        if n == 0 {
            return 1.0;
        }
        let lam = self.config.decay_lambda.clamp(0.0, 1.0);
        let mut w_sum = 0.0;
        let mut value_w = 0.0;
        for (i, s) in self.window.iter().enumerate() {
            // Newest entry last: it is 0 steps from newest, weight 1.0.
            let w = lam.powi((n - 1 - i) as i32) * s.weight;
            w_sum += w;
            value_w += w * s.value;
        }
        if w_sum <= f64::EPSILON {
            return 0.0;
        }
        value_w / w_sum
    }

    /// The number of samples in the window whose value is below the configured
    /// violation threshold.
    pub fn violation_count(&self) -> u32 {
        self.window
            .iter()
            .filter(|s| s.value < self.config.violation_threshold)
            .count() as u32
    }

    /// The level implied by the score and the violation floor, before any
    /// degrade-only latch or promotion gate. Mirrors OAP's
    /// `raw_privilege_level`.
    pub fn raw_level(&self) -> Level {
        let mut level = self.config.thresholds.level_of(self.score());
        if self.violation_count() >= self.config.violation_floor {
            level = Level::max_severity(level, Level::Restricted);
        }
        level
    }

    /// The effective level, accounting for direction.
    ///
    /// `DegradeOnly`: the worst of the raw level and the latched severity (never
    /// self-promotes; clear with [`restore_max`](Self::restore_max)).
    /// `Bidirectional`: the raw level, except promotion to `Full` requires at
    /// least `promote_min_samples` observations in the window.
    pub fn level(&self) -> Level {
        let raw = self.raw_level();
        match self.config.direction {
            Direction::DegradeOnly => Level::from_severity(raw.severity().max(self.stuck_severity)),
            Direction::Bidirectional => {
                if raw == Level::Full
                    && self.config.promote_min_samples > 0
                    && self.window.len() < self.config.promote_min_samples
                {
                    Level::Restricted
                } else {
                    raw
                }
            }
        }
    }

    /// Restore maximum trust: clear the window and the degrade-only latch. This
    /// is OAP's `human_restore` (an explicit human action, not a scorer
    /// transition).
    pub fn restore_max(&mut self) {
        self.stuck_severity = 0;
        self.window.clear();
    }

    /// Serialize the mutable state for durable persistence.
    pub fn snapshot(&self) -> WindowSnapshot {
        WindowSnapshot {
            samples: self.window.iter().copied().collect(),
            stuck_severity: self.stuck_severity,
        }
    }

    fn update_stuck(&mut self) {
        // The latch only applies in DegradeOnly; a Bidirectional scorer must be
        // free to promote, so it never accrues stuck severity.
        if self.config.direction == Direction::DegradeOnly {
            let raw = self.raw_level();
            self.stuck_severity = self.stuck_severity.max(raw.severity());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LevelThresholds;

    fn cfg(window_size: usize, decay_lambda: f64, violation_floor: u32) -> WindowConfig {
        WindowConfig {
            window_size,
            direction: Direction::DegradeOnly,
            decay_lambda,
            thresholds: LevelThresholds::default(),
            violation_threshold: 0.5,
            violation_floor,
            promote_min_samples: 0,
        }
    }

    #[test]
    fn violations_force_at_least_restricted() {
        // Port of OAP SC-007: score alone would be Full (0.8), but the
        // violation floor forces at least Restricted.
        let mut s = WindowScorer::new(cfg(10, 1.0, 2));
        for _ in 0..8 {
            s.record_aligned(true);
        }
        s.record_aligned(false);
        s.record_aligned(false);
        assert!((s.score() - 0.8).abs() < 1e-9);
        assert_eq!(s.raw_level(), Level::Restricted);
    }

    #[test]
    fn degrade_only_no_self_promotion() {
        // Port of OAP SC-008: monotonic latch, cleared only by restore_max.
        let mut s = WindowScorer::new(cfg(20, 1.0, 100));
        for _ in 0..10 {
            s.record_aligned(true);
        }
        for _ in 0..10 {
            s.record_aligned(false);
        }
        assert_eq!(s.raw_level(), Level::Restricted);
        assert_eq!(s.level(), Level::Restricted);
        for _ in 0..40 {
            s.record_aligned(true);
        }
        assert!((s.score() - 1.0).abs() < 1e-9);
        assert_eq!(s.raw_level(), Level::Full);
        assert_eq!(s.level(), Level::Restricted, "must not self-promote");
        s.restore_max();
        assert_eq!(s.level(), Level::Full);
    }

    #[test]
    fn threshold_crossing_read_only_and_suspended() {
        // Port of OAP: default decay 0.95, recent violations weighted more.
        let mut s = WindowScorer::new(cfg(10, 0.95, 100));
        for _ in 0..3 {
            s.record_aligned(true);
        }
        for _ in 0..7 {
            s.record_aligned(false);
        }
        assert_eq!(s.raw_level(), Level::ReadOnly);

        let mut s2 = WindowScorer::new(cfg(10, 0.95, 100));
        for _ in 0..10 {
            s2.record_aligned(false);
        }
        assert_eq!(s2.raw_level(), Level::Suspended);
    }

    #[test]
    fn weighted_window_favors_recent() {
        // Port of OAP: decay 0.5, window 4.
        let mut s = WindowScorer::new(cfg(4, 0.5, 100));
        s.record_aligned(true);
        s.record_aligned(true);
        s.record_aligned(false);
        s.record_aligned(false);
        let w = [0.125_f64, 0.25, 0.5, 1.0];
        let expected = (w[0] + w[1]) / w.iter().sum::<f64>();
        assert!((s.score() - expected).abs() < 1e-9);
    }

    #[test]
    fn weighted_samples_generalize_booleans() {
        // A graded 0.5 sample scores exactly halfway; booleans are the extremes.
        let mut graded = WindowScorer::new(cfg(4, 1.0, 100));
        for _ in 0..4 {
            graded.record(Sample::new(0.5));
        }
        assert!((graded.score() - 0.5).abs() < 1e-9);

        // Heavier weight pulls the score toward that sample.
        let mut weighted = WindowScorer::new(cfg(2, 1.0, 100));
        weighted.record(Sample::weighted(0.0, 1.0));
        weighted.record(Sample::weighted(1.0, 3.0));
        assert!((weighted.score() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn bidirectional_promotes_and_demotes() {
        let mut c = cfg(10, 1.0, 100);
        c.direction = Direction::Bidirectional;
        let mut s = WindowScorer::new(c);
        for _ in 0..10 {
            s.record_aligned(false);
        }
        assert_eq!(s.level(), Level::Suspended);
        // Recovery promotes back up (no latch).
        for _ in 0..10 {
            s.record_aligned(true);
        }
        assert_eq!(s.level(), Level::Full, "bidirectional promotes on recovery");
    }

    #[test]
    fn bidirectional_promote_min_samples_gates_full() {
        let mut c = cfg(10, 1.0, 100);
        c.direction = Direction::Bidirectional;
        c.promote_min_samples = 5;
        let mut s = WindowScorer::new(c);
        // Two perfect samples: score is Full, but too few observations to grant it.
        s.record_aligned(true);
        s.record_aligned(true);
        assert!((s.score() - 1.0).abs() < 1e-9);
        assert_eq!(
            s.level(),
            Level::Restricted,
            "Full gated until enough samples"
        );
        for _ in 0..3 {
            s.record_aligned(true);
        }
        assert_eq!(s.level(), Level::Full, "promoted once min samples reached");
    }

    #[test]
    fn snapshot_round_trip() {
        let mut s = WindowScorer::new(cfg(10, 0.95, 3));
        for i in 0..6 {
            s.record(Sample::new(if i % 2 == 0 { 1.0 } else { 0.2 }));
        }
        let snap = s.snapshot();
        let restored = WindowScorer::from_snapshot(s.config().clone(), snap);
        assert!((restored.score() - s.score()).abs() < 1e-12);
        assert_eq!(restored.level(), s.level());
        assert_eq!(restored, s);
    }

    #[test]
    fn empty_window_is_full() {
        let s = WindowScorer::with_defaults();
        assert!((s.score() - 1.0).abs() < 1e-9);
        assert_eq!(s.level(), Level::Full);
    }

    #[test]
    fn determinism_same_sequence_same_result() {
        let seq = [1.0, 0.0, 0.3, 0.9, 0.1, 1.0, 0.4];
        let build = || {
            let mut s = WindowScorer::new(cfg(5, 0.9, 3));
            for v in seq {
                s.record(Sample::new(v));
            }
            s
        };
        let a = build();
        let b = build();
        assert_eq!(
            a.score().to_bits(),
            b.score().to_bits(),
            "bit-identical score"
        );
        assert_eq!(a.level(), b.level());
    }
}

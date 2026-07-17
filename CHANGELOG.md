# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-07-14

### Added

- Initial release. `WindowScorer`: a rolling-window trust scorer that maps a
  stream of weighted `Sample`s to a graduated `Level`
  (Full/Restricted/ReadOnly/Suspended) via a decay-weighted score and a
  violation floor.
- `Direction::DegradeOnly` (monotonic latch, cleared by `restore_max`) and
  `Direction::Bidirectional` (promote and demote, with promotion to `Full`
  gated on `promote_min_samples`).
- `WindowSnapshot` + `from_snapshot` / `snapshot`: a persistence seam for
  durable, cross-session per-subject windows, with no storage in the library.
- Determinism: given the same sample sequence and config, `score()` and
  `level()` are reproducible (no wall clock), covered by a bit-identical
  determinism test.
- Extracted and relicensed Apache-2.0 (by the sole copyright holder) from the
  Open Agentic Platform's `crates/policy-kernel/coherence.rs`
  (`CoherenceScheduler`). Booleans generalize to weighted samples, the
  degrade-only latch becomes configurable direction, and a snapshot seam is
  added. See `NOTICE`.

[0.1.0]: https://github.com/statecrafting/trust-window/releases/tag/v0.1.0

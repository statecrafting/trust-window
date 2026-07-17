# trust-window

A rolling-window trust scorer with a graduated privilege output. Feed a stream
of weighted samples, read back a decay-weighted score and a privilege level. The
score-to-level mapping direction is configuration: latch-down-only (an agent
whose trust ratchets down until a human restores it) or bidirectional (an
autonomy ladder that promotes and demotes with observed behavior).

```rust
use trust_window::{Direction, Level, Sample, WindowConfig, WindowScorer};

let mut cfg = WindowConfig::default();      // window 50, OAP bands, degrade-only
cfg.direction = Direction::Bidirectional;   // promote and demote
let mut scorer = WindowScorer::new(cfg);

for _ in 0..50 {
    scorer.record(Sample::aligned(true));   // booleans are the special case
}
assert_eq!(scorer.level(), Level::Full);

// A graded, weighted signal (e.g. 1 minus normalized edit distance):
scorer.record(Sample::weighted(0.4, 2.0));
```

## What it is

- **`Sample { value, weight }`**: a graded observation in `[0, 1]`. A boolean
  aligned/violation is `Sample::aligned(bool)`, the special case that recovers
  OAP's original input exactly.
- **`WindowScorer`**: the pure, deterministic scorer. `score()` is a
  decay-weighted mean; `level()` maps it to `Full` / `Restricted` / `ReadOnly` /
  `Suspended` through configurable thresholds and a violation floor.
- **`Direction::DegradeOnly`**: the level latches to its worst and only recovers
  on an explicit `restore_max()`. **`Direction::Bidirectional`**: the level
  tracks the score both ways, with promotion to `Full` gated on
  `promote_min_samples` observations.
- **`WindowSnapshot` + `from_snapshot` / `snapshot`**: persist a per-subject
  window to your own store and rehydrate it, without the library knowing about a
  database.

## What it is not

It ships the scorer, not the ladder. Which subject a window belongs to (an
agent, a play, a channel), where snapshots live, when a promotion becomes an
autonomy change, and which signals feed the samples are all the consumer's
concern. High-stakes hard-pins ("always require review here regardless of
level") are consumer policy layered on top, not a scorer feature.

## Determinism

Given the same sample sequence and config, `score()` and `level()` are
reproducible: no wall clock, no hidden state. The window ages samples by
insertion order, not by time.

## Ecosystem

Part of the `statecrafting` reusable-primitive family, extracted from the Open
Agentic Platform (`crates/policy-kernel/coherence.rs`) and relicensed Apache-2.0
by the sole copyright holder (see `NOTICE`). This repo is self-governed by its
own `specs/` corpus, compiled by the pinned `spec-spine` library.

Licensed under Apache-2.0.

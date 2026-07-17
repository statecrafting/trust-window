---
id: "000-trust-window-bootstrap"
title: "trust-window bootstrap (rolling-window trust scorer)"
status: draft
created: "2026-07-14"
authors: ["trust-window"]
kind: tooling
implementation: pending
risk: low
summary: >
  Bootstrap spec for the trust-window repository: a single-crate rolling-window
  trust scorer that maps a stream of weighted samples to a graduated privilege
  level, degrade-only or bidirectional, deterministic and snapshot-persistable.
  Extracted (relicensed Apache-2.0 by the sole copyright holder) from the Open
  Agentic Platform's crates/policy-kernel/coherence.rs (CoherenceScheduler),
  generalizing booleans to weighted samples and the degrade-only latch to a
  configurable direction, and adding a snapshot seam. This spec establishes the
  crate skeleton and seeds this repo's own spec corpus, governed by the pinned
  spec-spine library.
depends_on: []
establishes:
  - { kind: file, path: "Cargo.toml" }
  - { kind: file, path: "src/lib.rs" }
  - { kind: file, path: "src/config.rs" }
  - { kind: file, path: "src/scorer.rs" }
references:
  - { unit: { kind: file, path: "README.md" }, role: context }
  - { unit: { kind: file, path: "NOTICE" }, role: context }
---

# 000: trust-window bootstrap

## 1. Purpose

trust-window is the run-time trust-scoring primitive of the `statecrafting`
reusable-primitive family. It answers one question deterministically: given a
window of graded observations about a subject's behavior, what privilege level
should it hold right now? The scorer is pure (no wall clock) and its state is
snapshot-serializable, so a consumer can keep a durable per-subject window and
rehydrate it across sessions.

This bootstrap spec exists so the repository has a governed seed: the crate
compiles, this corpus is non-empty, and spec-spine can dogfood it.

## 2. Scope

In scope, established here:

- `Sample` (graded, weighted observations; booleans are the special case),
  `Level` (the four-tier privilege ladder), `LevelThresholds`, `Direction`
  (`DegradeOnly` / `Bidirectional`), `WindowConfig`, and `WindowSnapshot`.
- `WindowScorer`: `record`, `score`, `violation_count`, `raw_level`, `level`,
  `restore_max`, and the `snapshot` / `from_snapshot` persistence seam.
- A determinism test asserting a bit-identical score for the same sample
  sequence, and ports of OAP's coherence tests as the regression guard.

Out of scope:

- **The ladder.** Which subject a window belongs to, where snapshots are
  stored, when a promotion becomes an autonomy change, which signals feed the
  samples, and any hard-pin policy are all the consumer's concern.
- **Confidence math beyond a minimum-samples gate.** Promotion to `Full`
  requires `promote_min_samples` observations; a fancier statistical test can
  replace the internal rule later without changing the interface.

## 3. Provenance

Extracted from OAP `crates/policy-kernel/coherence.rs`
(`CoherenceScheduler`, AGPL-3.0-or-later there), relicensed Apache-2.0 by the
sole copyright holder. OAP's source was already domain-neutral (zero coupling
hits); the extraction generalizes its direction and input rather than decoupling
it. The interim extraction record is
`chancery/docs/preliminary/00-extraction-overview.md` and `03-trust-window.md`;
a forthcoming OAP extraction spec formalizes the vend.

## 4. Established units

- `Cargo.toml`: the single-crate manifest, Apache-2.0, edition 2024,
  forbid-unsafe.
- `src/config.rs`: the value and configuration types.
- `src/scorer.rs`: `WindowScorer` and its tests.
- `src/lib.rs`: crate docs and the public re-export surface.

## 5. Notes on the generalization

OAP's `CoherenceScheduler` fed booleans (`aligned`), latched monotonically down
(`stuck_severity`, cleared by `human_restore`), and kept the window only in
memory. trust-window generalizes on three axes without changing the scoring
math for the boolean special case: (1) `Sample { value, weight }` replaces the
boolean, with `Sample::aligned(bool)` recovering OAP exactly; (2) `Direction`
makes the latch configurable, so a bidirectional autonomy ladder can promote;
(3) `WindowSnapshot` lets a consumer persist and rehydrate per-subject windows.
OAP re-consumes this as `Direction::DegradeOnly` with `Sample::aligned`, and its
coherence tests are the regression guard.

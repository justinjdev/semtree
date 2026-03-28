---
path: openspec/changes/srt-benchmark/specs/bench-quality/spec.md
type: file
content_hash: f402588f49800ff118e19c8e63e2e3b545e5ffee307472bcbd955cc38bca51b7
---

This specification defines requirements for a quality validation phase in the SRT (Semantic Resolution Tree) benchmark system. The quality phase performs five key integrity checks: verifying that directory routing tables include all child entries (children coverage), validating YAML frontmatter in `.sem/` records contains required fields, detecting stale records by recomputing and comparing content hashes, identifying orphaned `.sem/` records whose source files no longer exist, and confirming deterministic builds by verifying identical hashes across rebuilds. Each requirement includes detailed scenarios specifying the expected behavior and metrics to be reported, such as coverage fractions, error counts, and determinism failures.

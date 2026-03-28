---
path: openspec/changes/srt-benchmark/specs/bench-harness
type: directory
content_hash: 1a70494b7c4d2537a232826ae79c0eff05d67d3d0ceb065e2e639d50757fd30a
---

This directory contains specifications for a semtree benchmark harness, which is part of a larger SRT benchmarking initiative. The harness provides a CLI interface for performance testing different phases of the semtree system (build, quality, routing, incremental). The system is designed to collect timing metrics and log results to standardized TSV files for analysis.

## Children

- **spec.md**: Specification defining requirements for a semtree benchmark harness with `semtree bench <phase>` CLI command for performance testing, supporting individual or sequential phase execution with automatic timing collection and TSV logging

---
path: openspec/changes/srt-benchmark/specs/bench-build
type: directory
content_hash: 882f18e81c594f225e2c18de83464bc660190227f1a23c87a197cfa63f2c1258
---

This directory contains the specification for the build phase of the SRT (Semantic Resolution Tree) benchmarking system. The build phase measures indexer performance by conducting both full non-incremental builds and immediate incremental rebuilds on a benchmark repository. It tracks key metrics including wall-clock time, LLM API call counts, and total node counts to evaluate SRT indexer efficiency and incremental build optimization.

## Children

- **spec.md**: Specification defining requirements for the build phase of SRT benchmarking, including performance measurement protocols for full and incremental builds, metric collection standards, and testing procedures against a ~160 file benchmark repository

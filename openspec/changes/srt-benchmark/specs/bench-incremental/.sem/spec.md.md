---
path: openspec/changes/srt-benchmark/specs/bench-incremental/spec.md
type: file
content_hash: 6db68ce0610d4c0afbf3a2fbe39294aa1805acb7b9ea38ff4f42579f1e2cdeac
---

This specification defines requirements for benchmarking incremental rebuilds in the SRT (Semantic Resolution Tree) system. The spec outlines a test phase that modifies a deterministic set of files in a benchmark repository, triggers an incremental rebuild, and then measures performance and correctness. Key requirements include measuring rebuild time (`incr_rebuild_time_s`), counting re-summarized nodes (`nodes_resummarized`), verifying that only changed subtrees are processed while unchanged siblings retain original hashes, and validating correctness by recomputing all content hashes after the rebuild. The specification emphasizes that incremental rebuilds should be both faster than full rebuilds and produce identical results, with built-in mechanisms to restore the repository to its original state after testing.

---
path: openspec/changes/srt-benchmark/design.md
type: file
content_hash: 05e26f3a1f154f0582f89424aa2a5515f3dc9fa44d180d5d378c5631afe9a8a1
---

This design document outlines a benchmarking harness for evaluating the SRT (Semantic Resolution Trees) indexer system. The benchmark aims to measure four key aspects: build performance, structural quality of generated summaries, routing effectiveness, and incremental build correctness across pinned repository snapshots. The design specifies a Python module architecture that calls semtree library functions directly (avoiding CLI subprocess overhead), uses YAML files to define query sets with expected target files, and simulates agent routing behavior programmatically rather than using live agents. The system logs results in append-only TSV format and initially focuses on structural quality checks (ensuring summaries have proper metadata, all children are documented, hashes are fresh) rather than semantic accuracy evaluation.

---
path: openspec/changes/srt-benchmark/specs/bench-routing
type: directory
content_hash: afd4211eba5996cd33a54b76f911fde75c35ea6c86777f50cb36dda7831872e4
---

This directory contains specifications for benchmarking SRT (Semantic Resolution Tree) routing performance. The benchmark evaluates how effectively agents can navigate the SRT hierarchy to find relevant files by simulating the descent protocol with real query workloads. It measures routing accuracy through recall metrics and navigation efficiency by tracking the number of summary files accessed during traversal.

## Children

- **spec.md**: Defines requirements for benchmarking SRT routing performance through simulated agent descent, including query set formats, recall@k metrics, and reproducibility requirements with temperature=0 LLM calls

---
path: openspec/changes/srt-benchmark/tasks.md
type: file
content_hash: 60f4fd127dc9301f1dc979ab84a7103ac1ace2c9eef544f833f5f7b64db35486
---

This file is a comprehensive task breakdown for implementing an SRT (Semantic Resolution Trees) benchmarking suite. It defines a multi-phase benchmark system accessible via `semtree bench` that tests build performance, structural quality validation, routing effectiveness, and incremental rebuild efficiency across different repository sizes. The benchmark harness collects timing metrics, LLM call counts, and accuracy measures (like recall@k for routing queries) and logs results to TSV format. Key phases include build performance measurement, quality assurance checks for `.sem/` record integrity, routing simulation to test navigation effectiveness, and incremental rebuild validation. The system is designed to work with cached repository clones and includes comprehensive test coverage for each benchmarking component.

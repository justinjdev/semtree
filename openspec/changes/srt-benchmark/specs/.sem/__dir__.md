---
path: openspec/changes/srt-benchmark/specs
type: directory
content_hash: 7b115f3ed9cab6fe7defd86f5159c04704cae314a1e004395f79681874958e5c
---

This directory contains specifications for a comprehensive SRT (Semantic Resolution Tree) benchmarking system. The benchmarking suite is designed to evaluate different aspects of SRT performance including build times, routing accuracy, incremental rebuild efficiency, and overall system quality. Each subdirectory defines requirements for a specific benchmark phase that collectively provides end-to-end performance measurement of the SRT indexer and query system.

## Children

- **bench-build**: Specification for benchmarking SRT build performance, measuring indexer efficiency through full and incremental build timing, API call counts, and node generation metrics
- **bench-harness**: Specification for the benchmark CLI harness that orchestrates performance testing across different phases with standardized timing collection and TSV result logging
- **bench-incremental**: Specification for benchmarking incremental rebuild performance, validating that only changed portions of the tree are rebuilt while preserving unchanged subtree hashes
- **bench-quality**: Specification for quality validation phase that ensures SRT build integrity through routing table coverage, frontmatter validation, and deterministic build verification checks
- **bench-repos**: Specification for repository management system that handles cloning and caching of benchmark repositories at pinned commits across small, medium, and large size tiers
- **bench-routing**: Specification for benchmarking SRT routing performance through simulated agent descent, measuring navigation accuracy and efficiency using recall metrics

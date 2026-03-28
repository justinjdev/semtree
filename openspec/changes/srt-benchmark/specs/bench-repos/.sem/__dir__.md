---
path: openspec/changes/srt-benchmark/specs/bench-repos
type: directory
content_hash: e8f06de9f48c5f871b1006a01f26de0e8509e3055cc676c854ad38ea15ff7a0c
---

This directory contains the specification for the benchmark repository management component of the SRT benchmark system. The repository manager handles cloning and caching of benchmark repositories at pinned commits to ensure reproducible performance testing across different repository sizes. It supports a tiered approach to benchmarking with small, medium, and large repositories, providing deterministic benchmark environments through commit-based pinning rather than moving branch heads.

## Children

- **spec.md**: Defines the repository manager that clones and caches benchmark repos at pinned commits, supporting three size tiers (small <200 files, medium 200-1000 files, large >1000 files) with configuration via `bench/repos.yaml` and local caching in `bench/.repos/` to ensure reproducible benchmark environments

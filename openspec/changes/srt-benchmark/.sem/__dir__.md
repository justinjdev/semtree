---
path: openspec/changes/srt-benchmark
type: directory
content_hash: e2f49c19a32a7beb1ffedeedb96042903fc14fca6fa50695d39a7088111745d0
---

This directory contains the design and specifications for a comprehensive benchmarking system for the SRT (Semantic Resolution Trees) project. The benchmarking suite is designed to evaluate key aspects of SRT performance including build times, routing accuracy, incremental rebuild efficiency, and structural quality validation. The system will operate through a `semtree bench` CLI command and test across repositories of different sizes using pinned commits, with results logged in TSV format for analysis.

## Children

- **design.md**: Design document outlining the benchmarking harness architecture that measures build performance, structural quality, routing effectiveness, and incremental build correctness using direct Python module calls and programmatic agent simulation
- **proposal.md**: Proposal document for adding comprehensive benchmarking capabilities to validate that SRT summaries actually improve code navigation, addressing the current lack of empirical validation for the indexer system
- **specs**: Directory containing detailed specifications for each benchmark phase including build performance, quality validation, routing accuracy, incremental rebuild verification, and repository management components
- **tasks.md**: Comprehensive task breakdown for implementing the SRT benchmarking suite with multi-phase testing, timing metrics collection, and accuracy measurement through recall@k for routing queries

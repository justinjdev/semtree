---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/incremental-rebuild
type: directory
content_hash: 239444e014575eb09973f73b5e305be1a3d66d686d5eecaa8777e53aca97f782
---

This directory contains specifications for implementing incremental rebuild functionality in the SRT Python indexer. The specification focuses on hash-based change detection to avoid unnecessary re-summarization of unchanged files and directories. This optimization significantly improves build performance by only processing content that has actually changed while maintaining the correctness of the hierarchical summary structure.

## Children

- **spec.md**: Specification document defining requirements for hash-based incremental rebuilds in the SRT Python indexer, including content hash comparison, selective re-summarization, crash resumability, and force rebuild capabilities.

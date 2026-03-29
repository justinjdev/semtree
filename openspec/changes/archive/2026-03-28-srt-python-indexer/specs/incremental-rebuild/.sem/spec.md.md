---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/incremental-rebuild/spec.md
type: file
content_hash: 09aae7b7548267adbd266a8fac72d23b40a75271f9d312496460b0dd3deb5db1
---

This specification document defines requirements for implementing hash-based incremental rebuilds in an SRT Python indexer. The core functionality compares freshly computed content hashes against stored hashes in existing `.sem/` records to determine whether files need re-summarization, skipping unchanged files to avoid unnecessary LLM calls. When a file changes, both the file and its parent directories are re-summarized since directory hashes depend on their children. The system supports crash resumability (interrupted builds can be resumed safely) and includes a `--force` flag to override all freshness checks and regenerate everything. This incremental approach optimizes build performance by only processing changed content while maintaining the correctness of the hierarchical summary structure.

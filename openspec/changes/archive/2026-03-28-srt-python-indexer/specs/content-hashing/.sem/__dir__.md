---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/content-hashing
type: directory
content_hash: 588ce0855e6eb5ed8ce3d206f0f3ab07642bc72a3d4514b907aabb971b4795bf
---

This directory contains the content hashing specification for the SRT Python indexer implementation. The specification defines the deterministic hashing scheme used to track changes in the Semantic Resolution Tree, establishing SHA-256-based content hashing for both files and directories. This hashing system enables incremental rebuilds by detecting when content has changed at any level of the filesystem hierarchy.

## Children

- **spec.md**: Defines SHA-256 content hashing requirements for files (raw byte contents) and directories (canonical sorted child path-hash pairs), ensuring deterministic results and enabling incremental SRT rebuilds through hash-based change detection

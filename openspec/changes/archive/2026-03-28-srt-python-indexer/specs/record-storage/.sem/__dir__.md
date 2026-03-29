---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/record-storage
type: directory
content_hash: 3b7c031b057103964d699b1f0b57a057e5573f3925591c8fddb4bd2490b0e5c0
---

This directory contains specifications for record storage in the SRT (Semantic Resolution Tree) Python indexer implementation. It defines the fundamental storage format for how summary records are persisted as colocated Markdown files with YAML frontmatter in `.sem/` hidden directories. The specification establishes the core data structure requirements that enable git-native, zero-infrastructure summary hierarchies where each record contains path, type, and content hash metadata for incremental rebuild support.

## Children

- **spec.md**: Specification document defining record storage requirements for SRT summary records, including the colocated `.sem/` directory structure, YAML frontmatter format with path/type/content_hash fields, and incremental rebuild freshness comparison mechanisms

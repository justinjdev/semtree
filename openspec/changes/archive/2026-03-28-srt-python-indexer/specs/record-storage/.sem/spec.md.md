---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/record-storage/spec.md
type: file
content_hash: 9dea87d720395e0d2456557a87ba0e5f6201f23593800eb7cf69cf6c053de22d
---

This specification document defines the record storage requirements for an SRT (Semantic Resolution Tree) Python indexer. It establishes that summary records must be stored in colocated `.sem/` hidden directories alongside the code they describe, with directory records at `__dir__.md` and file records at `<filename>.md`. Each record uses YAML frontmatter containing `path`, `type`, and `content_hash` fields, delimited by `---` markers, followed by a Markdown summary body. The specification also requires the indexer to read existing records to extract stored content hashes for incremental rebuild freshness comparison. This defines the core storage format that enables the SRT system to maintain git-native, zero-infrastructure summary hierarchies.

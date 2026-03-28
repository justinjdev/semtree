---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/content-hashing/spec.md
type: file
content_hash: 74570a3512afa8e7d45beb6e8e42a6b7c361effc2772d13e543b81d6f80fcc38
---

This specification defines content hashing requirements for the SRT Python indexer. It establishes that files use SHA-256 hashes of their raw byte contents (stored as lowercase hex), while directories use SHA-256 hashes of a canonical string formed from sorted child path-hash pairs. The hashing scheme ensures deterministic results independent of processing order and enables incremental rebuilds by propagating hash changes upward through the directory tree when any descendant file is modified. The specification includes scenarios validating that identical files produce identical hashes and that directory hashes correctly reflect changes to their contents.

---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/tree-construction/spec.md
type: file
content_hash: 597c6eeb2fc0f2dfced3a798654174cc9f1c08b8263369a1cb135bc42ca3b8cd
---

This OpenSpec file defines requirements for tree construction in the SRT Python indexer. It specifies that the indexer must use post-order depth-first search traversal, processing all children before their parent directory and maintaining deterministic lexicographic ordering. The spec mandates exclusion of dotfiles, dot-directories, symbolic links, and binary files (detected by null bytes in the first 8192 bytes). All paths in the resulting tree are stored relative to the specified repository root, ensuring a clean hierarchical structure that mirrors the filesystem while filtering out irrelevant or problematic file types.

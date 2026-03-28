---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/tree-construction
type: directory
content_hash: b9f1fd0eac8f068d7f1ac67193f66788d2e9c348f22370ff383599d32fd470a9
---

This directory contains specifications for tree construction in the SRT Python indexer implementation. It defines the algorithmic requirements for how the indexer should traverse and build the semantic resolution tree structure. The specifications ensure deterministic behavior through post-order depth-first search and lexicographic ordering while excluding problematic file types.

## Children

- **spec.md**: OpenSpec file defining tree construction requirements including post-order DFS traversal, deterministic lexicographic ordering, and exclusion rules for dotfiles, dot-directories, symlinks, and binary files

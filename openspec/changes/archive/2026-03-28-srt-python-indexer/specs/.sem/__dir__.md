---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs
type: directory
content_hash: fd8cf5b83d7ea4daf8fceccea9cb9f6ec979c0a0344895084e569b0c4eefbea9
---

This directory contains the complete specification suite for the SRT (Semantic Resolution Tree) Python indexer implementation, archived from an OpenSpec change on March 28, 2026. The specifications define a comprehensive system for building git-native semantic summary hierarchies that mirror filesystem structure using colocated Markdown records. The indexer design emphasizes zero infrastructure dependencies, deterministic behavior through content hashing, and incremental rebuild capabilities for efficient maintenance of code repository summaries.

## Children

- **cli/**: CLI specification defining requirements for the `semtree` command-line tool including build commands, model selection, token limits, and error handling for constructing Semantic Resolution Trees
- **content-hashing/**: Specification for SHA-256-based deterministic hashing scheme for files and directories, enabling incremental rebuilds through hash-based change detection
- **incremental-rebuild/**: Requirements for hash-based incremental rebuild functionality to avoid unnecessary re-summarization of unchanged content while maintaining hierarchical summary correctness
- **record-storage/**: Core storage format specification defining how summary records are persisted as colocated Markdown files with YAML frontmatter in `.sem/` hidden directories
- **summarization/**: LLM-powered summarization component specification covering routing table generation, oversized file handling, provider interfaces, and error handling strategies
- **tree-construction/**: Algorithmic requirements for tree traversal and construction using post-order DFS, deterministic lexicographic ordering, and exclusion rules for problematic file types

---
path: openspec/changes/archive/2026-03-28-srt-python-indexer
type: directory
content_hash: 23e4b931601f70a6b62f4b4be267105eae850130d5068c0cd6a3292e1d796a53
---

This directory contains a complete archived OpenSpec change documenting the implementation of a Python-based indexer for Semantic Resolution Trees (SRT). The change implements a CLI tool called `semtree` that generates git-native semantic summary hierarchies by traversing repositories bottom-up and creating LLM-powered summaries stored as colocated Markdown records in `.sem/` directories. The implementation emphasizes deterministic behavior through SHA-256 content hashing, incremental rebuilds, and zero infrastructure dependencies while following the SRT specification from `docs/srt_v7.tex`.

## Children

- **cli/**: CLI specification defining requirements for the `semtree` command-line tool including build commands, model selection, token limits, and error handling for constructing Semantic Resolution Trees
- **content-hashing/**: Specification for SHA-256-based deterministic hashing scheme for files and directories, enabling incremental rebuilds through hash-based change detection  
- **incremental-rebuild/**: Requirements for hash-based incremental rebuild functionality to avoid unnecessary re-summarization of unchanged content while maintaining hierarchical summary correctness
- **record-storage/**: Core storage format specification defining how summary records are persisted as colocated Markdown files with YAML frontmatter in `.sem/` hidden directories
- **summarization/**: LLM-powered summarization component specification covering routing table generation, oversized file handling, provider interfaces, and error handling strategies
- **tree-construction/**: Algorithmic requirements for tree traversal and construction using post-order DFS, deterministic lexicographic ordering, and exclusion rules for problematic file types
- **design.md**: Architectural design document outlining implementation decisions for the Python SRT indexer, including package structure, traversal algorithms, content hashing approach, and LLM integration strategy
- **proposal.md**: Project proposal defining the implementation plan for the `semtree` CLI tool with deterministic filesystem traversal, LLM-generated summaries, and incremental rebuild capabilities
- **specs/**: Complete specification suite defining all components of the SRT Python indexer system with detailed requirements for each subsystem
- **tasks.md**: Completed implementation task breakdown covering 35 subtasks across 7 major components from project scaffolding to end-to-end integration testing

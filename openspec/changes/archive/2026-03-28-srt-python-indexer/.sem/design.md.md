---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/design.md
type: file
content_hash: 2467fa268c0e42bb2b833dbfbb231b45d9005af17033d923d04b5fbb6b819ce0
---

This is a design document for a Python implementation of the Semantic Resolution Tree (SRT) indexer described in the paper `docs/srt_v7.tex`. The document outlines architectural decisions for building a standalone CLI tool (`semtree build`) that generates `.sem/` summary directories by traversing repositories bottom-up, computing content hashes, and creating LLM-generated summaries. Key design choices include a flat package structure under `src/semtree/`, post-order filesystem traversal using `os.walk`, SHA-256 content hashing for incremental rebuilds, and Anthropic SDK integration with a provider abstraction layer. The implementation prioritizes correctness and simplicity over optimization, with sequential processing, manual YAML frontmatter handling, and configurable token limits for oversized files.

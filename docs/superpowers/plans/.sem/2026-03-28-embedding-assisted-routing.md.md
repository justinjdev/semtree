---
path: docs/superpowers/plans/2026-03-28-embedding-assisted-routing.md
type: file
content_hash: 132df33ad08b8362ecbbd616313153daf82967b824a251353b645d8b0e86d21a
---

This file is a comprehensive implementation plan for adding embedding-assisted routing to the Semantic Resolution Tree (SRT) system. The plan adds fastembed-based local embeddings to help pre-filter high fan-out directories using cosine similarity before LLM routing, stored as `.vec` sidecar files alongside existing `.sem/` summary records. It introduces two new CLI subcommands (`semtree embed` for standalone embedding generation and `semtree query` for cosine-ranked child retrieval) and integrates embedding computation into the main build pipeline with a `--no-embed` opt-out flag. The plan is structured as 8 detailed tasks covering dependency management, embedder module implementation with `.vec` I/O and freshness checking, CLI command development, build pipeline integration, navigation skill updates, and comprehensive testing. The implementation maintains the system's zero-infrastructure design by using local ONNX models and git-tracked vector files, with embeddings triggered automatically during builds but available as standalone tools for query-time routing assistance.

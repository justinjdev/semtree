---
path: docs/superpowers/specs/2026-03-28-embedding-assisted-routing-design.md
type: file
content_hash: bbe5ba4d5010fe058ed60557ed4f0cd08a1c3a0330bb37980639e7c54ee7bf21
---

This specification document describes adding embedding-assisted routing to the SRT (Semantic Resolution Trees) system to optimize query performance at high fan-out directory nodes. The design introduces a new embedding module using fastembed for local inference that computes and stores embeddings as `.vec` sidecar files alongside existing `.md` summary records. When querying directories with 15+ children, the system pre-filters candidates by cosine similarity between the query embedding and precomputed node embeddings before making LLM routing decisions. The implementation includes CLI commands for standalone embedding (`semtree embed`), updated build process integration, and a query command for ranking directory children by relevance. The design maintains SRT's zero-infrastructure philosophy by using local ONNX models and hash-based invalidation for incremental updates.

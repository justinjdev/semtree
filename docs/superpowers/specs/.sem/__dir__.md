---
path: docs/superpowers/specs
type: directory
content_hash: 39180a2ade0de7d9fc564818b38a1e0fbd20a0ea2726cb1a290aed855d529dea
---

This directory contains specification documents for the superpowers framework, focusing on enhancements to the Semantic Resolution Trees (SRT) system. Currently, it houses a design specification for embedding-assisted routing, which aims to optimize query performance in high fan-out directories by pre-filtering candidates using cosine similarity before making LLM routing decisions. The specification maintains SRT's zero-infrastructure philosophy while adding semantic search capabilities through local ONNX models.

## Children

- **2026-03-28-embedding-assisted-routing-design.md**: Design specification for adding embedding-assisted routing to SRT using fastembed for local inference, with `.vec` sidecar files and cosine similarity pre-filtering for directories with 15+ children

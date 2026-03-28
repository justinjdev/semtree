---
path: docs/superpowers/plans
type: directory
content_hash: 45a495dd85eb1052359909649a0ff51b34bf631a74ec2245ab80883345ee1199
---

This directory contains implementation plans for superpowers features. Currently, it holds a single comprehensive plan for adding embedding-assisted routing capabilities to the Semantic Resolution Tree (SRT) system. The plan outlines how to integrate local embeddings using fastembed to improve routing performance in high fan-out directories while maintaining the system's zero-infrastructure design philosophy.

## Children

- **2026-03-28-embedding-assisted-routing.md**: Implementation plan for adding fastembed-based local embeddings to pre-filter directories using cosine similarity, introducing new CLI subcommands and integrating embedding computation into the build pipeline while preserving git-native storage

---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/proposal.md
type: file
content_hash: bd14aa38f10e77d3069dad7f8e180ecc913e98352f9c139627745996dfc02202
---

This is a project proposal document that outlines the implementation plan for a Python CLI tool called `semtree` that builds Semantic Resolution Trees (SRT) for repositories. The tool performs deterministic post-order DFS filesystem traversal, generates LLM-powered summaries of files and directories, and stores them as colocated Markdown records in `.sem/` directories with YAML frontmatter containing path, type, and content hash metadata. Key features include git-style SHA-256 content hashing for incremental rebuilds, crash-resumable operation, and directory records with routing tables listing immediate children. The proposal addresses the need to implement the reference indexer specified in the SRT paper (`docs/srt_v7.tex`), creating a git-tracked summary hierarchy that mirrors the repository's filesystem structure.

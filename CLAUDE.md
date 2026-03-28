# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This repository implements **Semantic Resolution Trees (SRT)** — a deterministic summary hierarchy that mirrors a repository's filesystem, stored as colocated Markdown records tracked by git. The design is specified in `docs/srt_v7.tex`.

SRT is a **routing layer**, not a retrieval system. Summaries help agents decide where to read raw code; the agent always reads source files before answering. The key invariant: summaries route, code answers.

## Architecture (from the spec)

### Data Structure

- The SRT mirrors the filesystem: files are leaves, directories are internal nodes
- Each node stores a natural-language summary generated bottom-up from its children
- Summaries are colocated Markdown records inside `.srt/` hidden directories:
  - `<dir>/.srt/__dir__.md` — directory summary with routing table of all immediate children
  - `<dir>/.srt/<filename>.md` — file summary
- Each record has YAML frontmatter: `path` (repo-relative), `type` (file|directory), `content_hash` (git-style)

### Offline Build (Indexer)

- Post-order DFS traversal (children before parents)
- File hashes computed from full file contents; directory hashes from sorted `(child_path, child_hash)` pairs
- Summaries regenerated only when content hash changes (incremental rebuild)
- Ignores: symlinks, binary files, dotfiles, dot-directories
- Oversized files get placeholder: `summary unavailable: oversized file`
- Reference implementation is a Python indexer

### Query-Time Protocol

1. Read `.srt/__dir__.md` at the relevant directory (fall back to normal search if absent)
2. Scan routing table, select relevant children, skip irrelevant branches
3. Descend into selected children recursively
4. Read raw source files once candidates identified (~3-5 files)
5. Follow imports/grep/neighbors normally from there

### Optional: Embedding-Assisted Routing

- Pre-filter children by cosine similarity before LLM routing call
- Most useful for high-fan-out directories (30+ children)
- Stored in SQLite or serialized vectors in `.srt/`
- Same hash-based invalidation as summaries

## Key Design Constraints

- **Zero infrastructure**: no vector DB, no embedding service, no server — just files in git
- **Tool/model agnostic**: any LLM generates summaries, any agent consumes them
- **Git-native**: summaries version-controlled alongside code, visible in diffs and PRs
- **Graceful fallback**: if `.srt/` doesn't exist at a path, agent searches normally
- **Discovery-based scoping**: agent checks for `.srt/__dir__.md` at any path it visits; no hardcoded paths

## Build Commands

```bash
# Compile the paper
cd docs && pdflatex srt_v7.tex
```

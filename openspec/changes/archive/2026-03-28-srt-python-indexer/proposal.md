## Why

This repository contains the SRT paper (`docs/srt_v7.tex`) but no implementation. The paper specifies a Python reference indexer as the concrete deliverable. We need to build it: a CLI tool that walks a repository, generates bottom-up summaries via LLM, and stores them as colocated Markdown records in `.sem/` directories — the core artifact the paper defines.

## What Changes

- New Python CLI tool (`semtree`) that builds the SRT for a given repository
- Deterministic post-order DFS filesystem traversal (ignoring dotfiles, dot-directories, symlinks, binary files)
- File-level summarization via LLM (repo-relative path + full contents as input)
- Bottom-up directory summarization from child summaries
- Git-style content hashing for files (SHA-256 of contents) and directories (SHA-256 of sorted child path/hash pairs)
- Incremental rebuild: skip nodes whose content hash matches the stored hash
- Colocated `.sem/` Markdown records with YAML frontmatter (`path`, `type`, `content_hash`)
- Directory records include a `## Children` routing table listing every immediate child
- Oversized file handling: placeholder summary when file exceeds model context window
- Crash-resumable: partial builds can be re-run safely

## Capabilities

### New Capabilities

- `tree-construction`: Deterministic filesystem traversal, post-order DFS, node discovery, and ignore rules (dotfiles, symlinks, binaries, dot-directories)
- `content-hashing`: Git-style SHA-256 hashing for files (from contents) and directories (from sorted child path/hash pairs), used for freshness checks and incremental rebuilds
- `summarization`: LLM-powered summarization of file leaves and directory nodes using the minimal prompt templates from the paper, including oversized file handling
- `record-storage`: On-disk `.sem/` record format — YAML frontmatter + Markdown body, colocated with the code, directory records with `## Children` routing tables
- `incremental-rebuild`: Hash comparison against stored `content_hash` in frontmatter to skip unchanged nodes, with crash-resumable semantics
- `cli`: Command-line interface for building/rebuilding the SRT for a target repository

### Modified Capabilities

<!-- None — greenfield project -->

## Impact

- **New files:** Python package under `src/semtree/` (or similar), plus `pyproject.toml` for packaging
- **Dependencies:** An LLM SDK (Anthropic or OpenAI), PyYAML or equivalent for frontmatter, standard library for hashing/filesystem
- **Repository effect:** Running the tool on a repo will create `.sem/` directories throughout the target codebase — these should be committed to git per the paper's design
- **No effect on the paper itself** — this is the implementation the paper describes

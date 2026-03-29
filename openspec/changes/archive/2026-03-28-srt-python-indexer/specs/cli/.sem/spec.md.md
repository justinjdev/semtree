---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/cli/spec.md
type: file
content_hash: 47259c47251f5a0fe6797f947793a185094429ac88a7c5a5560819c7ccd36cca
---

This specification document defines CLI requirements for a `semtree` indexer tool that builds Semantic Resolution Trees (SRT) for repositories. The core functionality centers around a `build` command that constructs or incrementally updates SRT summaries, supporting both current directory and explicit path targeting. Key CLI features include model selection via `--model` flag (defaulting to Claude Sonnet 4), configurable token limits with `--max-tokens` (default 100,000), and force rebuild capability with `--force` flag. The tool provides progress feedback during builds and is designed to be installable via pip with a `semtree` command-line entry point. The specification covers error handling scenarios like non-existent paths and defines expected behavior for incremental vs. full rebuilds.

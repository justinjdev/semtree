---
path: openspec/specs/cli/spec.md
type: file
content_hash: 47259c47251f5a0fe6797f947793a185094429ac88a7c5a5560819c7ccd36cca
---

This specification file defines the CLI requirements for the `semtree` indexer tool that builds Semantic Resolution Trees. It specifies a `build` command that can construct or incrementally update SRT records for a repository, with support for explicit paths or defaulting to the current directory. The spec includes configuration flags for model selection (`--model`, defaulting to `claude-sonnet-4-20250514`), token limits (`--max-tokens`, defaulting to 100,000), and force rebuilds (`--force` to bypass incremental hash checks). It also requires progress output to stderr and defines that the package should be pip-installable with a `semtree` CLI entry point available on PATH.

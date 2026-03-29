---
path: openspec/changes/archive/2026-03-28-srt-python-indexer/specs/summarization/spec.md
type: file
content_hash: 30836d45e3b97e68f2a8317f4bc1bf56a475bf7293ed2bcd61dfce6283c0a4d1
---

This specification document defines requirements for LLM-powered summarization in an SRT (Semantic Resolution Tree) indexer system. The spec outlines how the indexer should generate natural-language summaries for both individual files and directories by making calls to an LLM, with directories receiving routing tables that mention all immediate children by name. Key implementation details include oversized file handling (files exceeding a token limit get placeholder summaries), a default LLM provider interface using the `claude` CLI in pipe mode, and graceful error handling with retries for failed summarization calls. The document uses a scenario-driven format to specify exact behaviors, such as how directory summaries must include every child in their routing tables and how the system should continue building even when individual LLM calls fail.

---
path: openspec/changes/srt-benchmark/specs/bench-build/spec.md
type: file
content_hash: c6c838378a3de145749f181fdb1f995d09e03eb6bc131c2569c00c7477d8a3ab
---

This specification defines requirements for the build phase of an SRT (Semantic Resolution Tree) benchmarking system. The build phase measures performance by conducting both a full non-incremental build and an immediate incremental rebuild on a benchmark repository, recording metrics like wall-clock time, LLM API call counts, and total node counts for comparison. Key requirements include cleaning any existing `.sem/` directories before the full build, using a force flag to ensure complete rebuilding regardless of prior state, and tracking LLM calls separately for full builds (which should equal the total nodes processed) versus incremental builds (which should be zero when no changes exist). The specification targets benchmarking against a repository with approximately 160 files across 30 directories, providing a standardized measure of SRT indexer performance and incremental build efficiency.

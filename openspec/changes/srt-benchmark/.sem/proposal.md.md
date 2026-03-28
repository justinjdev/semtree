---
path: openspec/changes/srt-benchmark/proposal.md
type: file
content_hash: bb02599d4b379d86fe010dcaaf6ce9d8c266710e8fe832292f159c93ae1af61d
---

This is a proposal document for adding a comprehensive benchmarking system to the SRT (Semantic Resolution Trees) project. The proposal addresses the need to measure whether SRT summaries actually improve code navigation for agents, since the current indexer builds `.sem/` records but lacks empirical validation. The document outlines six new benchmark capabilities: a main harness runner, build performance measurement, quality validation, routing accuracy testing, incremental rebuild verification, and benchmark repository management. The system would use four benchmark phases across pinned repositories of different sizes, with results logged to append-only TSV files, and would be accessible via a new `semtree bench` CLI command without modifying existing indexer code.

# Glamdring Benchmark Report (2026-03-29)

## Repository
- **Name:** glamdring (AI coding assistant in Go)
- **Files:** 296 source files
- **Packages:** 13 Go packages + SvelteKit site + OpenSpec docs
- **Queries:** 20 (7 focused, 7 module, 6 cross-cutting)

## Summary

| System | Best NDCG@10 | Avg NDCG | Hits | P50 Latency |
|--------|-------------|----------|------|-------------|
| Shire (FTS+RAG) | 0.973 | 0.669 | 20/20 | 0.1ms |
| SRT (embedding) | 0.939 | 0.340 | 10/20 | 1.4ms |
| ripgrep | 0.803 | 0.172 | 10/20 | 59.2ms |
| grep | 0.563 | 0.055 | 4/20 | 177.5ms |

## Per-Query Breakdown

| Query | Category | Question | SRT | Shire | rg | grep |
|-------|----------|----------|-----|-------|-----|------|
| q01 | focused | where is the TaskStatus type defined?... | 0.000 | 0.334 | 0.000 | 0.000 |
| q02 | focused | where is the PhaseRegistry type defined and what does i... | 0.000 | 0.687 | 0.000 | 0.000 |
| q03 | focused | where is the ReadTracker type defined?... | 0.000 | 0.822 | 0.000 | 0.000 |
| q04 | focused | where is the ResolveWorkflow function defined and what ... | 0.000 | 0.745 | 0.000 | 0.000 |
| q05 | focused | where is the model catalog defined and what models does... | 0.000 | 0.745 | 0.373 | 0.000 |
| q06 | focused | where are the built-in TUI themes defined?... | 0.939 | 0.861 | 0.102 | 0.078 |
| q07 | focused | where is the AdvancePhaseTool implemented?... | 0.000 | 0.822 | 0.000 | 0.000 |
| q08 | module | how does the permission system work for tool execution?... | 0.000 | 0.480 | 0.000 | 0.000 |
| q09 | module | how does the MCP server lifecycle management work?... | 0.000 | 0.973 | 0.000 | 0.000 |
| q10 | module | how does the multi-turn conversation session maintain s... | 0.659 | 0.624 | 0.758 | 0.394 |
| q11 | module | how does the hook system work for agent lifecycle event... | 0.914 | 0.598 | 0.000 | 0.000 |
| q12 | module | how does credential resolution work for multiple LLM pr... | 0.441 | 0.950 | 0.000 | 0.000 |
| q13 | module | how does the system prompt get assembled for agents?... | 0.691 | 0.408 | 0.000 | 0.000 |
| q14 | module | how does the task storage system work for persisting te... | 0.861 | 0.704 | 0.083 | 0.000 |
| q15 | cross-cutting | how do tools get registered and filtered by workflow ph... | 0.000 | 0.580 | 0.093 | 0.000 |
| q16 | cross-cutting | how does an MCP tool get adapted and exposed to the age... | 0.667 | 0.628 | 0.143 | 0.000 |
| q17 | cross-cutting | how does the read-before-write safety check work across... | 0.000 | 0.872 | 0.803 | 0.000 |
| q18 | cross-cutting | how does phase advancement interact with model switchin... | 0.320 | 0.724 | 0.468 | 0.000 |
| q19 | cross-cutting | how does the TUI handle streaming tool output and permi... | 0.587 | 0.467 | 0.059 | 0.059 |
| q20 | cross-cutting | how do tool decorators in teams enforce file path scopi... | 0.716 | 0.355 | 0.563 | 0.563 |

## Analysis

### SRT Strengths
- Module-level queries (q10-q14): avg 0.713 NDCG
- Cross-cutting queries (q16, q18-q20): avg 0.573 NDCG
- Sub-2ms latency (embedding-only, no LLM calls)

### SRT Weaknesses
- Focused symbol queries (q01-q07): avg 0.134 NDCG, 6/7 miss completely
- Summaries lack symbol names, preventing embedding match on type/function queries
- Shire wins here via FTS index that directly matches symbol names

### Shire Strengths
- Perfect hit rate (20/20) across all query categories
- Sub-millisecond latency via SQLite FTS5
- Symbol-level indexing catches focused queries SRT misses

### Optimization Opportunity
Enriching SRT summaries with key symbol names (type names, exported functions) would
close the focused-query gap without adding infrastructure. This is a prompt change
for the summarization step, not an architectural change.

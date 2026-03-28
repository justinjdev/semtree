# SRT Benchmark Report: Fellowship

**Date:** 2026-03-28
**Repo:** fellowship (~160 files, Go + SvelteKit + Markdown)
**Queries:** 15 (5 focused, 5 module, 5 cross-cutting)
**Cost model:** Opus 4.6 at $15/M input tokens

## Systems

| System | Method | Cost | Latency |
|---|---|---|---|
| SRT | Embedding beam-search over .sem/ summaries (fastembed bge-small-en-v1.5) | Token-based ($0.015/1K) | 0.008s mean |
| Shire | FTS5 + vector RAG over symbol/file index (MCP) | Free (local) | 0.003s mean |
| ripgrep | Keyword extraction + rg file search | Free (local) | 0.060s mean |
| grep | Keyword extraction + grep file search | Free (local) | 0.205s mean |

## Hypervolume (primary metric)

Higher hypervolume = larger favorable region in cost-latency-quality space.

| System | HV | 95% CI |
|---|---|---|
| **SRT** | **0.543** | [0.413, 0.665] |
| Shire | 0.404 | [0.215, 0.603] |
| ripgrep | 0.104 | [0.041, 0.193] |
| grep | 0.013 | [0.000, 0.033] |

## Best NDCG@10 per query

| Query | Category | SRT | Shire | rg | grep | Winner |
|---|---|---|---|---|---|---|
| q01: quest state machine | focused | 0.787 | 0.787 | 0.000 | 0.000 | tie |
| q02: gate guard blocking | focused | 0.787 | 0.787 | 0.000 | 0.000 | tie |
| q03: lembas prerequisite | focused | 0.000 | **1.000** | 0.000 | 0.000 | Shire |
| q04: SQLite schema | focused | **0.552** | 0.514 | 0.237 | 0.000 | SRT |
| q05: file tracking hook | focused | **0.956** | 0.918 | 0.000 | 0.000 | SRT |
| q06: hook system overview | module | 0.176 | **0.265** | 0.123 | 0.000 | Shire |
| q07: dashboard backend | module | **0.613** | 0.177 | 0.264 | 0.000 | SRT |
| q08: health monitoring | module | 0.627 | 0.000 | **0.787** | 0.497 | rg |
| q09: agent types | module | **0.689** | 0.000 | 0.363 | 0.390 | SRT |
| q10: errand system | module | **1.000** | **1.000** | 0.000 | 0.000 | tie |
| q11: gate submission e2e | cross-cutting | 0.407 | **0.521** | 0.000 | 0.000 | Shire |
| q12: CLI/dashboard/plugin coord | cross-cutting | **0.570** | 0.175 | 0.191 | 0.000 | SRT |
| q13: quest failure recording | cross-cutting | **0.674** | 0.000 | 0.000 | 0.000 | SRT |
| q14: bulletin board | cross-cutting | **1.000** | 0.000 | 0.387 | 0.000 | SRT |
| q15: plugin installation | cross-cutting | **0.443** | 0.000 | 0.124 | 0.000 | SRT |

**Wins: SRT 8, Shire 3, rg 1, ties 3**

## Hypervolume by query category

| System | Focused | Module | Cross-cutting |
|---|---|---|---|
| SRT | 0.561 | **0.570** | **0.498** |
| Shire | **0.791** | 0.284 | 0.137 |
| ripgrep | 0.030 | 0.215 | 0.067 |
| grep | 0.000 | 0.040 | 0.000 |

## SRT control parameter analysis

**By beam_width (mean NDCG@10 across all queries and settings):**
| beam | mean | max |
|---|---|---|
| 1 | 0.089 | 1.000 |
| 2 | 0.138 | 1.000 |
| 3 | 0.160 | 1.000 |
| 5 | 0.165 | 1.000 |

**By max_depth:**
| depth | mean | max |
|---|---|---|
| 1 | 0.005 | 0.144 |
| 2 | 0.044 | 0.689 |
| 3 | 0.259 | 1.000 |
| 100 | 0.244 | 1.000 |

## Pareto frontier diagnostics

Most SRT queries have only 2-3 Pareto-optimal points due to low cost variation in embedding-only routing. One query with sufficient frontier points:

- **q09** (agent types): 4 Pareto points, initial ascent = 73.1, knee at $0.037, flattening rate = 13.1

Richer frontier diagnostics require an LLM-oracle routing variant where per-level cost actually varies with the number of routing calls.

## Observations

1. **SRT dominates module-level and cross-cutting queries.** Hierarchical summaries compress subsystem understanding that flat retrieval must reconstruct indirectly. This confirms the paper's central hypothesis.

2. **Shire dominates focused queries.** Symbol search excels when the query targets a specific function or type by name. SRT's summary-based routing is a weaker signal for these.

3. **Depth = 3 is optimal for this repo.** Fellowship has 4-5 levels of nesting; depth=3 reaches the right files without over-exploring. Unlimited depth (100) is slightly worse due to candidate list dilution.

4. **Beam width has diminishing returns past 3.** Going from beam=1 to beam=3 doubles mean NDCG; beam=5 adds only 3% more.

5. **grep/rg fail on semantic queries.** Keyword matching can't do structural reasoning. rg's only win (q08: health monitoring) came from the keyword "eagles" matching the directory name literally.

6. **SRT's one miss (q03: lembas) is a summary quality issue.** The lembas prerequisite module's summary doesn't surface strongly enough during embedding routing. Shire finds it via exact symbol name matching. This suggests SRT and symbol search are complementary.

7. **SRT routing is extremely fast.** Mean latency 0.008s per operating point, with full root-to-leaf descent completing in under 0.1s. The embedding model loads once and amortizes across all queries.

## Raw data

- `results_full.tsv`: 13,500 metric records across 4 systems
- Control grids: SRT (96 settings), grep (48), ripgrep (24), Shire (12)
- Total operating points: 2,700 (15 queries x 180 settings)

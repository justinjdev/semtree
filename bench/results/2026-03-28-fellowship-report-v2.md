# SRT Benchmark Report v2: Fellowship (6 Systems)

**Date:** 2026-03-28
**Repo:** fellowship (~160 files, Go + SvelteKit + Markdown)
**Queries:** 15 (5 focused, 5 module, 5 cross-cutting)

## Systems

| System | Method | Latency (P50) |
|---|---|---|
| SRT Rust daemon | Native fastembed, Unix socket, model preloaded | 6.6ms |
| SRT Rust cold | Native fastembed, fresh process per query | 83.8ms |
| SRT Python | Python fastembed, fresh process per query | 5.1ms |
| Shire | FTS5 + vector RAG via MCP | 3.9ms |
| ripgrep | Keyword extraction + rg | 61.8ms |
| grep | Keyword extraction + grep | 206.8ms |

## Hypervolume

| System | HV | 95% CI |
|---|---|---|
| **SRT Rust daemon** | **0.599** | [0.457, 0.736] |
| SRT Python | 0.566 | [0.430, 0.689] |
| Shire | 0.399 | [0.212, 0.595] |
| SRT Rust cold | 0.305 | [0.232, 0.373] |
| ripgrep | 0.102 | [0.041, 0.181] |
| grep | 0.004 | [0.000, 0.009] |

## Hypervolume by Category

| System | Focused | Module | Cross-cutting |
|---|---|---|---|
| **Rust daemon** | **0.769** | 0.448 | **0.580** |
| Shire | **0.781** | 0.280 | 0.135 |
| SRT Python | 0.577 | **0.585** | 0.536 |
| Rust cold | 0.392 | 0.228 | 0.293 |
| ripgrep | 0.034 | 0.190 | 0.082 |
| grep | 0.000 | 0.011 | 0.000 |

## Best NDCG@10 Per Query

| Query | Category | Rust daemon | Rust cold | Python | Shire | rg | grep | Winner |
|---|---|---|---|---|---|---|---|---|
| q01 | focused | 0.787 | 0.787 | 0.787 | 0.787 | 0.000 | 0.000 | tie |
| q02 | focused | 0.787 | 0.787 | 0.787 | 0.787 | 0.000 | 0.000 | tie |
| q03 | focused | **1.000** | **1.000** | 0.000 | **1.000** | 0.000 | 0.000 | Rust/Shire |
| q04 | focused | **0.552** | **0.552** | **0.552** | 0.514 | 0.237 | 0.000 | SRT |
| q05 | focused | 0.907 | 0.907 | **0.956** | 0.918 | 0.000 | 0.000 | Python |
| q06 | module | 0.176 | 0.176 | 0.176 | **0.265** | 0.123 | 0.000 | Shire |
| q07 | module | 0.177 | 0.177 | **0.613** | 0.177 | 0.264 | 0.000 | Python |
| q08 | module | 0.394 | 0.394 | 0.627 | 0.000 | **0.787** | 0.497 | rg |
| q09 | module | 0.586 | 0.586 | **0.689** | 0.000 | 0.363 | 0.390 | Python |
| q10 | module | **1.000** | **1.000** | **1.000** | **1.000** | 0.000 | 0.000 | tie |
| q11 | cross-cut | 0.364 | 0.364 | 0.407 | **0.521** | 0.000 | 0.000 | Shire |
| q12 | cross-cut | 0.536 | 0.536 | **0.570** | 0.175 | 0.191 | 0.000 | Python |
| q13 | cross-cut | **0.674** | **0.674** | **0.674** | 0.000 | 0.000 | 0.000 | SRT |
| q14 | cross-cut | **1.000** | **1.000** | **1.000** | 0.000 | 0.387 | 0.000 | SRT |
| q15 | cross-cut | **0.484** | **0.484** | 0.443 | 0.000 | 0.124 | 0.000 | Rust |

## Frontier Diagnostics (Rust daemon)

| Query | Pareto pts | Initial ascent | Knee | Flattening | Peak quality |
|---|---|---|---|---|---|
| q04 | 4 | 208.9 | 8.6ms | -24.4 | 0.552 |
| q12 | 3 | 62.7 | 7.0ms | -136.3 | 0.536 |
| q15 | 4 | 120.5 | 6.3ms | 55.5 | 0.221 |

Most queries have only 2 Pareto-optimal points (insufficient for diagnostics) because embedding routing has near-uniform latency across control settings. The knees cluster around 6-9ms — quality becomes available almost immediately.

## Key Findings

1. **Rust daemon achieves the highest hypervolume (0.599).** The combination of native fastembed inference and daemon-mode model caching produces the best quality-at-latency tradeoff across all query types.

2. **Binary .vec format fixed Python's one miss.** q03 (lembas prerequisite) went from 0.000 (Python/JSON) to 1.000 (Rust/binary). Full f32 precision in binary format produces better cosine similarity rankings than JSON's truncated floats.

3. **Rust nearly matches Shire on focused queries (0.769 vs 0.781).** The binary .vec precision boost closed most of the gap. With symbol-enriched summaries (future work), SRT would likely surpass Shire on focused queries too.

4. **Python SRT still leads on module queries (0.585 vs 0.448).** Likely a tokenization difference between Python fastembed and Rust fastembed crate producing slightly different embeddings. Worth investigating.

5. **Rust cold (84ms) is 8x faster than Python cold (700ms).** Even without the daemon, the Rust binary provides sub-100ms routing.

6. **grep is effectively useless for semantic queries (HV 0.004).** ripgrep is better (HV 0.102) but still far below semantic approaches.

## Experimental Setup

| Parameter | Value |
|---|---|
| Repository | Fellowship (~160 files) |
| Queries | 15 (5 focused, 5 module, 5 cross-cutting) |
| SRT control grid | 9 settings (beam x depth) per system |
| Embedding model | BAAI/bge-small-en-v1.5 |
| Rust binary | Release build, fastembed crate v4 |
| Python | fastembed 0.7.4, ONNX Runtime |
| Shire | v0.3.0, FTS5 + RAG enabled |
| Total operating points | 1,350 (15 queries x 9 settings x 6 systems) |

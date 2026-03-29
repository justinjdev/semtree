# Turborepo Parameter Sweep Report (2026-03-29)

## Repository
- **Name:** turborepo (Vercel's monorepo build system)
- **Crates:** 59 Rust crates + JS/TS packages
- **Queries:** 40 (14 focused, 14 module, 12 cross-cutting)
- **Crate coverage:** 30+ crates targeted by queries

## Sweep Configuration

Grid: `beam_width` x `max_depth` x `beam_policy` = 5 x 4 x 2 = 40 configs

| Parameter | Values |
|-----------|--------|
| beam_width | 2, 3, 5, 7, 10 |
| max_depth | 3, 5, 7, 10 |
| beam_policy | uniform, waterfill |

Total route calls: 40 configs x 40 queries = 1600

## Best Config

| Parameter | Value |
|-----------|-------|
| beam_width | 7 |
| max_depth | 5 |
| beam_policy | uniform |
| **Avg NDCG@10** | **0.628** |
| **Hit rate** | **38/40** |
| **P50 latency** | **27.1ms** |

## Top Configs

| Rank | BW | MD | Policy | NDCG | Hits | P50ms |
|------|----|----|--------|------|------|-------|
| 1 | 7 | 5 | uniform | 0.628 | 38/40 | 27.1 |
| 2 | 10 | 5 | waterfill | 0.628 | 38/40 | 28.6 |
| 3 | 10 | 5 | uniform | 0.622 | 38/40 | 39.9 |
| 4 | 7 | 7 | uniform | 0.621 | 38/40 | 32.4 |
| 5 | 5 | 5 | uniform | 0.583 | 35/40 | 15.6 |

## Pareto Frontier (NDCG vs Latency)

| BW | MD | Policy | NDCG | Hits | P50ms |
|----|----|--------|------|------|-------|
| 2 | 5 | uniform | 0.363 | 24/40 | 8.3 |
| 3 | 5 | uniform | 0.434 | 27/40 | 10.3 |
| 5 | 5 | uniform | 0.583 | 35/40 | 15.6 |
| 7 | 5 | uniform | 0.628 | 38/40 | 27.1 |

## Parameter Analysis

### beam_width
Diminishing returns after 7. bw=10 adds 30% latency for +0 NDCG.

| BW | Avg NDCG | Best NDCG | Avg Hits | Avg P50ms |
|----|----------|-----------|----------|-----------|
| 2 | 0.252 | 0.363 | 16.5 | 7.8 |
| 3 | 0.314 | 0.434 | 20.0 | 9.1 |
| 5 | 0.403 | 0.583 | 24.9 | 13.0 |
| 7 | 0.452 | 0.628 | 27.9 | 20.5 |
| 10 | 0.464 | 0.628 | 29.0 | 35.6 |

### max_depth
md=3 is catastrophically bad. md=5 is optimal; 7 and 10 add nothing.

| MD | Avg NDCG | Best NDCG | Avg Hits | Avg P50ms |
|----|----------|-----------|----------|-----------|
| 3 | 0.002 | 0.002 | 1.9 | 10.2 |
| 5 | 0.505 | 0.628 | 30.9 | 17.4 |
| 7 | 0.501 | 0.621 | 30.9 | 20.5 |
| 10 | 0.501 | 0.621 | 30.9 | 20.8 |

### beam_policy: Uniform vs Waterfill
Uniform wins at bw=2-7. Waterfill ties or slightly wins only at bw=10, with lower latency.

| BW | Uniform NDCG | Waterfill NDCG | Delta | Uni P50 | WF P50 |
|----|-------------|---------------|-------|---------|--------|
| 2 | 0.363 | 0.310 | -0.052 | 8.3ms | 7.5ms |
| 3 | 0.434 | 0.403 | -0.031 | 10.3ms | 8.6ms |
| 5 | 0.583 | 0.493 | -0.091 | 15.6ms | 12.0ms |
| 7 | 0.628 | 0.583 | -0.044 | 27.1ms | 16.3ms |
| 10 | 0.622 | 0.628 | +0.006 | 39.9ms | 28.6ms |

(Matched at md=5 for clarity.)

## Per-Query Results (best config: bw=7, md=5, uniform)

| Query | Cat | NDCG | Question (truncated) |
|-------|-----|------|----------------------|
| q01 | focused | 0.734 | task hash computed for cache key |
| q02 | focused | 0.951 | CacheMultiplexer coordinates local/remote |
| q03 | focused | 0.852 | EngineBuilder constructs task graph |
| q04 | focused | 0.958 | turbo.json parsed into Rust types |
| q05 | focused | 0.470 | PackageManager trait and supported managers |
| q06 | focused | 0.883 | GlobWatcher monitors filesystem |
| q07 | focused | 0.918 | DaemonServer gRPC service |
| q08 | module | 0.704 | remote cache HTTP artifact upload/download |
| q09 | module | 0.605 | codemod migration discovers and applies |
| q10 | module | 0.559 | run command executes tasks with caching |
| q11 | module | 0.730 | daemon file watching tracks package changes |
| q12 | module | 0.482 | graph utilities detect cycles |
| q13 | module | 0.487 | config merges turbo.json/env/CLI |
| q14 | module | 0.000 | prune creates sparse monorepo subset |
| q15 | x-cut | 0.409 | env var config flows to task hash |
| q16 | x-cut | 0.000 | daemon accelerates run with pre-computed hashes |
| q17 | x-cut | 0.639 | workspace discovery feeds task graph |
| q18 | x-cut | 0.247 | dry run produces summaries without running |
| q19 | x-cut | 0.855 | cache signature authentication |
| q20 | x-cut | 0.172 | lockfile changes affect discovery/hashing |
| q21 | focused | 0.629 | ImportTraceType controls turbo-trace imports |
| q22 | focused | 0.947 | Berry lockfile DescriptorResolver |
| q23 | focused | 0.777 | RepoGitIndex uses gix-index |
| q24 | focused | 0.861 | TaskId vs TaskName |
| q25 | focused | 0.838 | microfrontends proxy trie routing |
| q26 | focused | 0.905 | make_retryable_request retry strategies |
| q27 | focused | 0.816 | PackageTaskEventBuilder hashes telemetry |
| q28 | module | 0.723 | boundaries tag-based import rules |
| q29 | module | 0.535 | scope --filter with git change detection |
| q30 | module | 0.613 | process crate child lifecycle/shutdown |
| q31 | module | 0.892 | auth SSO login with callback server |
| q32 | module | 0.659 | Lockfile trait abstracts npm/pnpm/berry/bun |
| q33 | module | 0.905 | microfrontends proxy WebSocket upgrade |
| q34 | module | 0.861 | otel OTLP metric export |
| q35 | x-cut | 0.517 | child process -> vt100 parser -> TUI pane |
| q36 | x-cut | 0.434 | turbo-trace imports -> boundaries enforcement |
| q37 | x-cut | 0.564 | shim locates local turbo binary |
| q38 | x-cut | 0.862 | task executor + run cache + OutputWatcher |
| q39 | x-cut | 0.828 | git changed files -> SCM -> scope -> query API |
| q40 | x-cut | 0.540 | Unix signals -> listeners -> process shutdown |

### By Category

| Category | Avg NDCG | Hits |
|----------|----------|------|
| Focused (14) | 0.824 | 14/14 |
| Module (14) | 0.668 | 13/14 |
| Cross-cutting (12) | 0.506 | 10/12 |

## Bugs Fixed During This Sweep

1. **Waterfill budget starvation**: global budget model starved deeper levels; replaced with per-directory ambiguity scaling
2. **Daemon connection leak**: missing `shutdown(SHUT_WR)` caused zombie connections; added idle timeouts

# SRT Evaluation Framework (v9)

**Date:** 2026-03-28
**Status:** Approved
**Spec reference:** `srt_v9.tex` Section 6 (Evaluation Framework)
**Builds on:** `openspec/changes/srt-benchmark/design.md` (practical harness)

## Summary

Update the SRT benchmark harness to implement v9's formal evaluation framework. The routing phase sweeps a control grid per system, recording (cost, latency, quality) per query per setting. A new analysis phase computes 3D Pareto frontiers, dominated hypervolume, budget/latency slices, and frontier shape diagnostics. Adds a grep/glob baseline for comparison.

## Motivation

The existing benchmark design measures SRT at a single fixed operating point. v9 argues that the right evaluation target is the *response surface* — the set of operating points a system can realize in cost-latency-quality space. This requires sweeping control grids and analyzing the geometry of the resulting frontiers, not just point estimates.

## Design

### Phases

| Phase | Source | Description |
|---|---|---|
| **build** | Existing | Measure full build time, LLM calls, node count |
| **quality** | Existing | Structural correctness: children coverage, frontmatter validity, hash consistency, orphan detection |
| **routing** | Reworked | Sweep control grid per system, record (cost, latency, quality) per query per setting |
| **incremental** | Existing | Modify file, rebuild, verify only changed subtree re-summarized |
| **analysis** | New | Pareto frontiers, hypervolume, frontier slices, shape diagnostics |

Phases 1-4 collect raw data into `results.tsv`. Phase 5 consumes that data.

### Control grids

**SRT:**
- `token_budget`: [1000, 2000, 5000, 10000, 20000, 50000]
- `beam_width`: [1, 2, 3, 5] (children selected per node)
- `max_depth`: [1, 2, 3, unlimited]
- `embed_threshold`: [None, 0.3, 0.5, 0.7] (cosine pre-filter cutoff)

**Grep/glob baseline:**
- `token_budget`: [1000, 2000, 5000, 10000, 20000, 50000]
- `max_files`: [3, 5, 10, 20]
- `strategy`: ["grep_only", "glob_then_grep"]

### Per (query, system, setting) measurements

- `cost_usd`: estimated dollar cost (LLM calls * per-call rate)
- `latency_s`: wall-clock seconds
- `ndcg@10`: against labeled ground truth (graded relevance)
- `tokens_loaded`: total tokens in context
- `llm_calls`: count of LLM routing calls

### Query set format

```yaml
queries:
  - question: "How does the quest state machine work?"
    category: focused  # focused | module | cross-cutting
    relevant:
      - path: cli/internal/state/state.go
        relevance: 3  # 3=primary, 2=supporting, 1=tangential
      - path: cli/internal/hooks/guard.go
        relevance: 2
```

- `relevance` grades: 3=primary, 2=supporting, 1=tangential (for NDCG)
- `category`: focused | module | cross-cutting (for per-category reporting)
- Target: ~5 focused, ~5 module-level, ~5 cross-cutting for fellowship

### Results format

Routing phase rows:

```
timestamp	phase	repo	system	query_id	control_json	metric	value
2026-03-28T12:00:00	routing	fellowship	srt	q03	{"beam":3,"depth":3}	ndcg@10	0.85
2026-03-28T12:00:00	routing	fellowship	srt	q03	{"beam":3,"depth":3}	cost_usd	0.002
2026-03-28T12:00:00	routing	fellowship	srt	q03	{"beam":3,"depth":3}	latency_s	1.3
2026-03-28T12:00:00	routing	fellowship	srt	q03	{"beam":3,"depth":3}	tokens_loaded	4200
2026-03-28T12:00:00	routing	fellowship	srt	q03	{"beam":3,"depth":3}	llm_calls	3
```

Build/quality/incremental phases use existing format (no query_id or control_json).

### Analysis module (`bench/analysis.py`)

Pure stateless functions consuming arrays, returning arrays. No file I/O.

**Core:**
- `pareto_prune(points) -> frontier` — extract non-dominated (cost, latency, quality) points
- `normalize_to_utility(frontier, global_ranges) -> normalized` — u_c = 1 - c̃, u_ℓ = 1 - ℓ̃, u_a = ã
- `hypervolume(frontier) -> float` — volume of dominated region, reference (0,0,0)
- `budget_slice(frontier, latency_band) -> curve` — quality vs cost at fixed latency
- `latency_slice(frontier, cost_band) -> curve` — quality vs latency at fixed cost

**Frontier diagnostics:**
- `initial_ascent(curve) -> float` — slope near minimum cost/latency
- `knee_location(curve, tau) -> float` — cost where marginal gain drops below τ
- `flattening_rate(curve) -> float` — post-knee decay of marginal gain

**Reporting:**
- `workload_hypervolume(per_query_hvs) -> (mean, ci_low, ci_high)` — bootstrap confidence intervals
- `category_hypervolume(per_query_hvs, categories) -> dict` — focused/module/cross-cutting

### Grep/glob baseline (`bench/baseline.py`)

Simulates an agent without SRT summaries:
- Given a query, use keyword extraction + glob/grep to find candidate files
- Controlled by `max_files` and `strategy` parameters
- Records same (cost, latency, quality) measurements as SRT
- Cost model: grep is free; if strategy uses LLM for keyword extraction, count that call

### Benchmark repo

Fellowship pinned at a specific commit (existing decision). Future tiers deferred.

## File inventory

| File | Action | Responsibility |
|---|---|---|
| `bench/__init__.py` | Create | Package init |
| `bench/harness.py` | Create | Phase runner, timing, results TSV I/O |
| `bench/repos.py` | Create | Repo cloning, pinning, caching |
| `bench/repos.yaml` | Create | Pinned repos config |
| `bench/quality.py` | Create | Structural correctness checks |
| `bench/routing.py` | Create | Simulated descent with control grid sweep |
| `bench/baseline.py` | Create | Grep/glob baseline system |
| `bench/build_phase.py` | Create | Build cost measurement |
| `bench/incremental.py` | Create | Incremental rebuild measurement |
| `bench/analysis.py` | Create | Pareto, hypervolume, frontier diagnostics |
| `bench/queries/fellowship.yaml` | Create | Labeled query set |
| `src/semtree/cli.py` | Modify | Add `semtree bench` subcommand |
| `openspec/changes/srt-benchmark/design.md` | Modify | Update with v9 framework |
| `tests/test_analysis.py` | Create | Unit tests for analysis functions |
| `tests/test_routing_bench.py` | Create | Tests for routing phase with mocked LLM |

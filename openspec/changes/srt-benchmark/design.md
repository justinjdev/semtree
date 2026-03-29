## Context

The SRT indexer builds `.sem/` summary records but we have no way to evaluate whether the output is useful. The paper (Section 6) defines a formal evaluation framework, but a practical benchmark harness comes first — something we can run today against real repos to measure build cost, output quality, and routing effectiveness.

Shire's benchmarking system (`autoresearch.rs`) provides a proven reference: pinned repos, phased benchmarks, direct library calls, append-only results log. We adapt that architecture for SRT's different concerns (summary quality rather than query latency).

## Goals / Non-Goals

**Goals:**
- Reproducible benchmarks across pinned repos at known commits
- Four phases measuring different aspects: build cost, structural quality, routing accuracy, incremental correctness
- Results logging for tracking improvements over time
- CLI integration: `semtree bench <phase>`
- Call library functions directly (not subprocess to CLI)

**Non-Goals:**
- ~~Full paper evaluation framework~~ (now implemented: Pareto, hypervolume, frontier diagnostics)
- Automated optimization loop (shire's autooptimize — future work after benchmarks are stable)
- Benchmarking against other tools (Claude Context, Shire, MCP Context Manager — requires integrating those tools)
- Production-grade statistical analysis (confidence intervals, significance tests)

## Decisions

### 1. Benchmark repos: start with fellowship, add tiers later

For v1, use the fellowship repo we're already testing with. It's a known codebase with good structure. Pinning at a specific commit ensures reproducibility.

Future tiers:
- Small: fellowship (~160 files, Go + Svelte + Markdown)
- Medium: a mid-size OSS repo (~500 files)
- Large: turborepo or similar (~1000+ files)

**Why not start with 3 repos:** We need the harness working first. One repo is enough to validate the benchmark design. Adding repos is mechanical once the harness exists.

### 2. Harness architecture: Python module calling semtree internals directly

The harness imports `semtree.walker`, `semtree.hasher`, `semtree.builder`, etc. directly — no subprocess overhead. This isolates what we're measuring (indexer performance) from CLI/process noise.

```
bench/
  __init__.py
  harness.py      # Phase runner, timing, results collection
  repos.py        # Repo cloning, pinning, caching
  quality.py      # Structural quality checks
  routing.py      # Routing accuracy evaluation
  queries/        # Query sets per repo (YAML files)
    fellowship.yaml
```

**Why not a separate binary:** Python is fine for benchmarking LLM-bound work. The bottleneck is API call latency, not harness overhead.

### 3. Query sets: YAML files with expected targets

Each benchmark repo has a query set defining questions and their expected target files:

```yaml
queries:
  - question: "How does the quest state machine work?"
    expected_files:
      - cli/internal/state/state.go
      - cli/internal/hooks/guard.go
    expected_dirs:
      - cli/internal/state
      - cli/internal/hooks
```

The routing phase reads `.sem/__dir__.md` at root, follows the routing table based on query relevance, and checks whether the descent reaches the expected files.

**Why YAML over Python fixtures:** Query sets should be human-readable and editable. Non-developers (or future agents) should be able to add queries without writing code.

### 4. Routing evaluation: simulate agent descent, don't use a live agent

Rather than spawning a Claude session and watching it navigate, we simulate the routing protocol programmatically:
1. Read root `__dir__.md`, extract children descriptions
2. Use an LLM call to select relevant children (same oracle as the paper)
3. Descend into selected children
4. Record which files are reached

This is cheaper and more reproducible than live agent testing. The LLM call for child selection is the same one an agent would make.

**Why not live agent testing:** Non-deterministic, expensive, hard to automate. Simulated descent tests the artifact (summaries), not the agent's discipline in following the protocol.

### 5. Results logging: append-only TSV matching shire's pattern

```
timestamp	phase	repo	metric	value	notes
2026-03-28T12:00:00	build	fellowship	build_time_s	180.5	initial build
2026-03-28T12:00:00	build	fellowship	llm_calls	241
2026-03-28T12:00:00	quality	fellowship	children_coverage	0.98	2 children missing
2026-03-28T12:00:00	routing	fellowship	recall@5	0.85	17/20 queries hit target
```

**Why TSV over JSON/SQLite:** Matches shire's proven pattern. Human-readable, appendable, diffable, greppable. No dependencies.

### 6. Quality checks: structural, not semantic

The quality phase checks mechanical correctness, not whether summaries are "good":
- Every child of a directory appears in its `__dir__.md` `## Children` section
- YAML frontmatter has required fields (path, type, content_hash)
- content_hash matches freshly computed hash (no staleness)
- No orphan `.sem/` records (record exists but source file doesn't)
- Deterministic: two builds produce identical hashes
- File type matches (file records say `type: file`, dir records say `type: directory`)

Semantic quality (are summaries accurate?) is harder to automate and is deferred.

## Risks / Trade-offs

- **Routing evaluation depends on LLM non-determinism** → Use temperature=0 and fixed model for reproducibility. Accept some variance; report multiple runs.
- **Query sets are manually authored** → Small initial set (10-20 queries for fellowship). Quality matters more than quantity for v1.
- **No baseline comparison yet** → First build the harness measuring SRT alone. Adding grep/glob baseline is a follow-up.
- **Fellowship is the only bench repo initially** → Results may not generalize. Mitigated by adding repos later.

## Open Questions

- Should routing evaluation use the same model as the indexer, or a cheaper one for cost reasons?
- How many routing queries per repo is "enough" for a meaningful signal?
- Should we track LLM API cost ($) in addition to call count?

## v9 Evaluation Framework Update

The benchmark harness now implements the full v9 evaluation framework from the paper, adding a 5th analysis phase and significant extensions to routing evaluation.

### Analysis Phase (5th phase)

The `bench/analysis.py` module consumes `results.tsv` and computes:

- **Pareto pruning**: Extracts non-dominated points from (cost, latency, quality) triples
- **Normalization to utility**: Maps raw metrics to [0,1] utility coordinates (cost and latency inverted)
- **Hypervolume**: Dominated hypervolume in utility space with reference point (0,0,0), using inclusion-exclusion
- **Budget slices**: Quality-vs-cost curves at fixed latency bands
- **Latency slices**: Quality-vs-latency curves at fixed cost bands
- **Frontier geometry diagnostics**: Initial ascent slope, knee location (smallest resource where marginal gain < tau), flattening rate (second-difference decay)
- **Workload hypervolume**: Mean hypervolume with bootstrap 95% CI across per-query measurements
- **Category hypervolume**: Mean hypervolume broken down by query category (focused, module, cross-cutting)

### Control Grids

**SRT control grid** sweeps:
- `beam_width`: [1, 2, 3, 5]
- `max_depth`: [1, 2, 3, 100 (unlimited)]
- `token_budget`: [1000, 2000, 5000, 10000, 20000, 50000]

**Baseline control grid** sweeps:
- `max_files`: [3, 5, 10, 20]
- `strategy`: ["grep_only", "glob_then_grep"]
- `token_budget`: [1000, 2000, 5000, 10000, 20000, 50000]

### Updated Results Format

The TSV now includes additional columns for multi-system comparison:

```
timestamp	phase	repo	system	query_id	control_json	metric	value
```

- `system`: "srt" or "baseline" — enables side-by-side comparison
- `query_id`: Links metrics to specific queries (e.g., "q03")
- `control_json`: JSON-encoded control settings for the run (e.g., `{"beam_width":3,"max_depth":3,"token_budget":5000}`)

### Query Set Format

Query sets now include graded relevance and categories:

```yaml
queries:
  - id: q01
    question: "How does the quest state machine track phase transitions?"
    category: focused  # focused | module | cross-cutting
    relevant:
      - path: cli/internal/state/state.go
        relevance: 3  # 3=primary, 2=supporting, 1=tangential
```

Categories enable per-category hypervolume analysis to identify where routing works best/worst.

### Grep/Glob Baseline

`bench/baseline.py` implements a keyword-extraction grep baseline for comparison:
- Extracts keywords from questions (stopword filtering)
- Runs `grep -rl` across source files
- Ranks by match count
- Reports as `system=baseline` in results TSV with `cost_usd=0.0`

## Why

The indexer builds `.sem/` records but we have no way to measure whether the summaries actually help agents navigate code. Without benchmarks we can't answer the paper's central empirical question: does the SRT improve routing enough to justify its build cost? We need a harness that measures build cost, summary quality, routing accuracy, and incremental correctness across repos of different sizes.

## What Changes

- New benchmark harness (`bench/`) that exercises the indexer and evaluates output quality
- Pinned benchmark repos at specific commits for reproducible evaluation
- Four benchmark phases: build, quality, routing, incremental
- Query sets with expected file targets for routing accuracy measurement
- Results logging (append-only TSV) for tracking experiments over time
- CLI command: `semtree bench <phase> [--repo <name>]`

## Capabilities

### New Capabilities

- `bench-harness`: Benchmark runner that orchestrates phases, manages bench repos, collects metrics, and writes results to TSV. CLI integration via `semtree bench` command.
- `bench-build`: Build phase — measures wall-clock time, LLM call count, node count, and token cost for full and incremental builds across benchmark repos.
- `bench-quality`: Quality phase — structural correctness checks on `.sem/` output: every child mentioned in parent routing table, frontmatter fields valid, hashes match content, no orphan records, deterministic rebuild produces same hashes.
- `bench-routing`: Routing phase — given a query set with expected target files, measures whether summary-guided descent reaches the right files. Compares files-opened and tokens-loaded with SRT vs. without (grep/glob baseline).
- `bench-incremental`: Incremental phase — modifies files in a bench repo, rebuilds, verifies only the changed subtree is re-summarized, measures rebuild time and correctness.
- `bench-repos`: Benchmark repo management — clone, pin at commit, cache locally. Three size tiers (small/medium/large) for testing at different scales.

### Modified Capabilities

<!-- None — benchmark system is additive -->

## Impact

- **New files:** `bench/` directory with harness code, query sets, and repo configs
- **New CLI command:** `semtree bench` subcommand added to `cli.py`
- **New file:** `results.tsv` at repo root (append-only experiment log, gitignored)
- **Dependencies:** None new — uses existing semtree internals directly
- **No changes to existing indexer code** — benchmarks consume it, don't modify it

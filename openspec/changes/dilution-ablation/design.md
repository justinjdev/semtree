## Context

The SRT routing protocol descends a tree using embedding cosine similarity to select beam_width children at each level. The existing bench infrastructure (`bench/routing.py`) runs a control grid over beam_width x max_depth x token_budget, measuring NDCG@10, cost, latency, tokens_loaded, and llm_calls. The `simulate_descent` function already tracks which files are reached but does not record per-level statistics about beam composition.

The hyperparameter model paper (Section 4, eq. 4-5) defines two dilution models:
1. Log-dilution: D(b, d) = sum w_l * log(1 + n_l(b)) where n_l is beam size at level l
2. Ratio model: D'(b, d) = sum w_l * rho_l(b) where rho_l is the fraction of irrelevant nodes in beam at level l

The objective U = G - mu*D - lambda*C trades off gain G against dilution penalty D and cost C. The ablation tests mu=0 (no dilution penalty) vs the two models.

## Goals / Non-Goals

**Goals:**
- Instrument routing descent to capture per-level telemetry: n_l (candidates seen), beam selected, rho_l (irrelevant fraction given ground truth)
- Implement log-dilution and ratio-dilution penalty computation
- Run three-way comparison (no-penalty, log-dilution, ratio) and measure impact on retrieval metrics
- Add precision, recall, MRR to the existing metric set (currently only NDCG@10)
- Results output in the existing TSV format for compatibility with `bench/analysis.py`

**Non-Goals:**
- Optimizing or tuning the weight parameters w_l and mu (this experiment identifies which model form to use; tuning is a follow-up)
- Implementing adaptive beam allocation (water-filling) -- separate change
- Modifying the Rust route command behavior at query time (this is a benchmark-only change)
- Changing the embedding model or summarization pipeline

## Decisions

### 1. Per-level telemetry struct

Add a `LevelTelemetry` dataclass to `bench/routing.py` containing:
- `depth`: level index
- `n_candidates`: total children available at this level
- `n_selected`: beam width used (may differ from requested if fewer children)
- `selected_paths`: which paths were selected
- `rho_l`: irrelevant fraction (computed post-hoc against ground truth relevant set)

**Rationale**: The telemetry is computed during descent (n_candidates, n_selected, selected_paths) and enriched post-hoc (rho_l requires ground truth). This separates routing mechanics from evaluation.

**Alternative considered**: Computing rho_l inline during descent. Rejected because the select_fn should not need access to ground truth -- that would contaminate the routing simulation.

### 2. Dilution penalty as post-hoc scoring, not routing modification

The dilution penalty D does not change which nodes are selected during routing. It is a post-hoc score computed from telemetry to evaluate whether the beam configuration led to diluted candidate sets. The ablation compares the correlation between penalty scores and retrieval quality, not between penalty-modified routing and unmodified routing.

**Rationale**: The paper's objective U = G - mu*D - lambda*C is for beam allocation optimization, not runtime re-ranking. The ablation measures whether D or D' better predicts retrieval degradation.

**Alternative considered**: Using dilution penalty to prune candidates during routing. Rejected because this conflates the measurement with the intervention -- we need to measure first, then decide whether to use it for pruning.

### 3. Three experimental conditions sharing the same descent traces

Run `simulate_descent` once per (query, control) pair and compute all three penalty scores from the same telemetry. This avoids 3x the routing cost and ensures apples-to-apples comparison.

**Rationale**: The descent trajectory is identical across conditions because the penalty does not modify routing. We only need one trace.

### 4. Extend Python bench (not Rust CLI) for the experiment

The Python bench already has the full routing simulation, control grid, metric collection, and query loading. The Rust `bench` command only runs quality checks. Add the dilution ablation as a Python bench phase.

**Rationale**: The Python bench is the experiment harness; the Rust CLI is the production tool. Experiments belong in the harness.

**Alternative considered**: Adding a dilution ablation mode to the Rust `bench` subcommand. Rejected because the Rust bench currently only does quality checks, and the Python bench already has the full routing simulation pipeline with `simulate_descent`, `SelectFn`, and the control grid.

### 5. Uniform weights w_l = 1.0 as the baseline

Start with uniform level weights (w_l = 1 for all l). The ablation is about model form (log vs ratio vs none), not weight tuning.

**Rationale**: Weight tuning is a follow-up. Using uniform weights isolates the model form comparison.

## Risks / Trade-offs

- [Risk: rho_l computation depends on ground truth quality] The relevant file annotations in query YAML files may be incomplete, making rho_l noisy. -> Mitigation: Use existing curated query files (fellowship.yaml, glamdring.yaml) which have manually verified relevant file lists.
- [Risk: Post-hoc penalty analysis may not generalize to runtime beam allocation] The penalty scores measure correlation, not causation. -> Mitigation: This is acknowledged as a limitation; the ablation informs which model form to use in the optimization, not whether to use it at runtime.
- [Risk: Small query set limits statistical power] Current query files may have few queries. -> Mitigation: Report per-query breakdowns and bootstrap confidence intervals using existing `workload_hypervolume` machinery from `bench/analysis.py`.

## Open Questions

- Should we sweep mu values (e.g., 0.1, 0.5, 1.0, 2.0) to see sensitivity, or focus purely on model form comparison with fixed mu=1.0?
- Should the CLI `bench` command gain a `--phase routing` option to invoke the Python bench, or keep them as separate entry points?

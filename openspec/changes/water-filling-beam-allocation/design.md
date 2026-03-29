## Context

The `route_directory` function in `cli/src/embedder.rs` performs top-down beam search through the SRT. At each level it:
1. Loads child vectors from `.sem/*.vec` files
2. Ranks children by cosine similarity to the query
3. Selects top-k via `adaptive_beam` (uniform beam_width with gap-based extension)
4. Queues selected directories for next-level descent

The current `adaptive_beam` function uses a single `beam_width` parameter applied identically at every level. The hyperparameter paper proves this is suboptimal: Proposition 1 shows beam should be allocated proportional to alpha_l = B_l * m_l per level.

## Goals / Non-Goals

**Goals:**
- Implement water-filling beam allocation as described in Proposition 1
- Measure per-level difficulty (branching factor and ambiguity) during descent
- Expose the policy choice via `--beam-policy` flag
- Preserve exact current behavior under `--beam-policy uniform`
- Report per-level allocation diagnostics in route output

**Non-Goals:**
- Confidence-gated widening (Section 4 of the paper) — separate change
- Early termination optimization — separate change
- Changing the embedding model or similarity computation
- Offline alpha_l calibration from historical queries

## Decisions

### 1. Two-pass vs single-pass allocation

**Decision:** Single-pass with lookahead estimation.

Water-filling requires knowing alpha_l for all levels upfront to distribute the budget. But during top-down descent, we don't know future levels until we explore them. Two approaches:

- **Two-pass**: First pass estimates alpha_l at each level (cheap: just count children and compute similarity spread), second pass does the actual routing with allocated beams.
- **Single-pass with budget remainder**: Estimate alpha_l at the current level, allocate proportionally from remaining budget, descend.

The two-pass approach would require loading all `.sem/` directories upfront, which contradicts the lazy-descent model. Instead, use single-pass: at each level, compute alpha_l for the current level and allocate from the remaining token/beam budget. This is a greedy approximation of the optimal water-filling but works naturally with the existing BFS descent.

**Refinement:** To improve on pure greedy, use a lookahead heuristic: estimate remaining levels from max_depth and assume average alpha for unseen levels. This lets the allocator reserve budget for deeper levels rather than spending it all at the root.

### 2. Ambiguity measure m_l

**Decision:** Use interquartile range (IQR) of cosine similarity scores among siblings.

Options considered:
- **Variance of similarity scores**: Simple but sensitive to outliers (one very dissimilar child skews it)
- **IQR of similarity scores (Q75 - Q25)**: Robust to outliers, captures the "middle spread" of scores
- **Entropy of softmax(scores)**: Information-theoretic but adds complexity and softmax temperature tuning

IQR is cheap to compute (we already have all scores from `cosine_rank`), robust, and interpretable. High IQR means scores are spread out (easy to distinguish children); low IQR means scores are clustered (ambiguous, need wider beam).

Note: ambiguity is *inversely* related to IQR — when scores are clustered (low IQR), it's hard to distinguish children, so m_l should be high. Define m_l = 1.0 - IQR (clamped to [0.1, 1.0]) so that ambiguous levels get higher alpha_l.

### 3. Budget model

**Decision:** Express budget as total beam units (sum of beam widths across levels), not tokens.

The paper uses token budget T with per-level cost c_l. For the embedding-only router, the dominant cost is the number of children examined, not tokens. Using beam units simplifies the model: the user specifies `--beam-width N` as total budget (reinterpreted under waterfill policy), and the allocator distributes N * max_depth total beam units across levels.

Total budget B_total = beam_width * max_depth (or beam_width * actual_depth for the greedy variant). At each level l, allocate b_l = B_remaining * (alpha_l / sum_alpha) where sum_alpha is estimated.

### 4. Integration point

**Decision:** Add a `BeamPolicy` enum and a new `waterfill_beam` function alongside the existing `adaptive_beam`.

```
enum BeamPolicy { Uniform, WaterFill }
```

In `route_directory`, dispatch on policy:
- `Uniform`: call `adaptive_beam` as today
- `WaterFill`: call `waterfill_beam` which takes the ranked scores, branching factor, remaining budget, and estimated remaining levels

This keeps the existing code path untouched and makes the new logic independently testable.

### 5. RouteLevel diagnostics

**Decision:** Add optional fields to `RouteLevel`:

- `branching_factor: Option<usize>` — B_l (number of children at this level)
- `ambiguity: Option<f32>` — m_l (computed ambiguity score)
- `allocated_beam: Option<usize>` — b_l (beam width allocated by waterfill)

These are `Option` so that uniform policy doesn't need to populate them, preserving backward compatibility in JSON output.

### 6. CLI flag design

**Decision:** `--beam-policy <uniform|waterfill>` with default `uniform`.

Considered `--waterfill` as a boolean flag but an enum is more extensible (could add `adaptive-waterfill`, `confidence-gated` later). The `--beam-width` parameter is reinterpreted under waterfill: it becomes the per-level base budget rather than a fixed width.

## Risks / Trade-offs

- **Greedy approximation vs optimal**: Single-pass greedy allocation is not truly optimal water-filling. It may over-allocate at shallow levels if they happen to be hard. Mitigation: the lookahead heuristic reserves budget for deeper levels, and in practice SRT trees are shallow (3-6 levels).

- **Ambiguity estimation noise**: IQR on a small number of children (e.g., 3-5) may be noisy. Mitigation: clamp m_l to [0.1, 1.0] so no level gets zero beam or extreme over-allocation.

- **Behavioral change risk**: None under default. Water-filling is opt-in via `--beam-policy waterfill`. Existing users see no change.

- **Daemon protocol change**: Adding `beam_policy` to the JSON params is additive. The daemon should default to `uniform` if the field is absent, preserving backward compatibility with older clients.

## Open Questions

- Should `--beam-width` under waterfill represent the *total* budget or the *average per-level* budget? Current design: average per-level (so total = beam_width * depth). This keeps the magnitude similar to uniform mode.
- Is IQR the right ambiguity measure, or should we experiment with entropy? Can defer to benchmarking.

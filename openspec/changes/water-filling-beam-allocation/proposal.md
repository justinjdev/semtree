## Why

The `semtree route` command uses adaptive beam search but allocates beam width uniformly across tree levels. The SRT hyperparameter optimization paper (docs/srt_hyperparam_model.tex, Proposition 1) proves that per-level allocation proportional to alpha_l = B_l * m_l (branching factor times ambiguity) outperforms uniform allocation, especially when difficulty varies across levels. This is the SRT analogue of water-filling in information theory. Implementing this brings the router in line with the theoretical optimum.

## What Changes

- Add per-level difficulty estimation to the router: compute branching factor B_l (child count) and ambiguity m_l (similarity spread among siblings) at each level during descent
- Implement water-filling beam allocation: distribute total beam budget across levels proportional to alpha_l, so harder levels (high fan-out, high ambiguity) get wider beams and easy levels are pruned aggressively
- Add a `--beam-policy` CLI flag to the `route` command: `uniform` (current behavior, default) vs `waterfill` (new allocation)
- Extend the `RouteLevel` output struct to include per-level diagnostics: branching factor, ambiguity score, and allocated beam width
- Propagate `beam-policy` through the daemon/server protocol

## Capabilities

### New Capabilities
- `water-filling-beam`: Per-level beam allocation using the water-filling algorithm from Proposition 1. Covers difficulty estimation (branching factor, ambiguity), budget distribution, and the two-pass routing protocol.

### Modified Capabilities
- `cli`: Add `--beam-policy` flag to the `route` command and propagate through daemon protocol.

## Impact

- **Code**: `cli/src/embedder.rs` (route_directory, adaptive_beam), `cli/src/main.rs` (CLI args, daemon params), `cli/src/server.rs` (daemon protocol)
- **API**: New `--beam-policy` flag; `RouteLevel` struct gains optional diagnostic fields. Existing behavior preserved under `--beam-policy uniform` (default).
- **Dependencies**: None. Uses existing cosine similarity infrastructure to compute ambiguity.
- **Risk**: Low. Default remains uniform allocation; water-filling is opt-in.

## 1. Data Types and CLI Plumbing

- [x] 1.1 Add `BeamPolicy` enum (`Uniform`, `WaterFill`) with clap `ValueEnum` derive to `cli/src/embedder.rs` (or a shared types location)
- [x] 1.2 Add `--beam-policy` flag to the `Route` command variant in `cli/src/main.rs` with default `Uniform`
- [x] 1.3 Add optional diagnostic fields to `RouteLevel` struct: `branching_factor: Option<usize>`, `ambiguity: Option<f32>`, `allocated_beam: Option<usize>`
- [x] 1.4 Pass `beam_policy` through to `route_directory` (update function signature)
- [x] 1.5 Add `beam_policy` to the daemon JSON protocol in `cli/src/server.rs`; default to `"uniform"` when absent

## 2. Ambiguity Measurement

- [x] 2.1 Implement `compute_ambiguity(scores: &[f32]) -> f32` function: compute IQR of scores, return m_l = clamp(1.0 - IQR, 0.1, 1.0); return 0.5 for fewer than 4 scores
- [x] 2.2 Add unit tests for `compute_ambiguity`: clustered scores, spread scores, fewer than 4 children, empty input, single child

## 3. Water-Filling Allocator

- [x] 3.1 Implement `waterfill_beam(ranked: &[(String, f32)], branching_factor: usize, ambiguity: f32, remaining_budget: usize, remaining_levels: usize) -> (Vec<(String, f32)>, usize)` returning selected children and beam used
- [x] 3.2 Implement budget computation: alpha_l = B_l * m_l, allocate b_l = max(1, round(remaining_budget * alpha_l / (alpha_l + avg_alpha * (remaining_levels - 1))))
- [x] 3.3 Add unit tests for `waterfill_beam`: hard level gets wider beam, easy level gets narrow beam, minimum beam of 1, budget not exceeded, single-level case

## 4. Router Integration

- [x] 4.1 Update `route_directory` to accept `BeamPolicy` parameter
- [x] 4.2 In the descent loop, when policy is `WaterFill`: compute ambiguity from ranked scores, compute alpha_l, call `waterfill_beam`, track remaining budget across levels
- [x] 4.3 Populate `RouteLevel` diagnostic fields (branching_factor, ambiguity, allocated_beam) when policy is `WaterFill`
- [x] 4.4 When policy is `Uniform`, preserve existing `adaptive_beam` behavior exactly (diagnostic fields remain `None`)

## 5. Output and Display

- [x] 5.1 Update the route output formatting in `main.rs` to display per-level diagnostics when present (beam allocated, branching factor, ambiguity)
- [x] 5.2 Update daemon response serialization to include new optional `RouteLevel` fields

## 6. Testing

- [x] 6.1 Add integration test: `--beam-policy uniform` produces identical output to current behavior (no `--beam-policy` flag)
- [x] 6.2 Add integration test: `--beam-policy waterfill` runs without error on a test SRT tree
- [x] 6.3 Verify daemon protocol backward compatibility: request without `beam_policy` field uses uniform

## 1. Per-Level Telemetry

- [x] 1.1 Add `LevelTelemetry` dataclass to `bench/routing.py` with fields: depth, n_candidates, n_selected, selected_paths, rho_l
- [x] 1.2 Modify `simulate_descent` to build and return a list of `LevelTelemetry` per level visited (extend `DescentResult` with `level_telemetry` field)
- [x] 1.3 Compute `rho_l` post-hoc: given selected_paths and the ground truth relevant set, determine which selected paths are on a path to a relevant leaf (ancestor check)
- [x] 1.4 Add unit tests for rho_l computation: all relevant, none relevant, mixed

## 2. Dilution Penalty Functions

- [x] 2.1 Implement `log_dilution_penalty(telemetry, weights)` -> float: sum(w_l * log(1 + n_selected_l))
- [x] 2.2 Implement `ratio_dilution_penalty(telemetry, weights)` -> float: sum(w_l * rho_l)
- [x] 2.3 Add unit tests for both penalty functions with known inputs matching spec scenarios

## 3. Additional Retrieval Metrics

- [x] 3.1 Implement `precision(retrieved, relevant_set)` -> float in `bench/routing.py`
- [x] 3.2 Implement `recall(retrieved, relevant_set)` -> float in `bench/routing.py`
- [x] 3.3 Implement `mrr(retrieved, relevant_set)` -> float in `bench/routing.py`
- [x] 3.4 Add unit tests for precision, recall, MRR covering edge cases (empty retrieved, empty relevant, no overlap)

## 4. Ablation Experiment Runner

- [x] 4.1 Add `run_dilution_ablation` function to `bench/routing.py` that runs the control grid once per (query, control) and computes all three conditions from shared telemetry
- [x] 4.2 Encode dilution condition in the `system` field as "srt/no_penalty", "srt/log_dilution", "srt/ratio_dilution"
- [x] 4.3 Emit precision, recall, MRR, NDCG@10, log_dilution_D, ratio_dilution_D, n_candidates_mean, rho_mean as metrics per condition
- [x] 4.4 Support incremental TSV output via results_path parameter (reuse existing `append_results`)

## 5. CLI Integration

- [x] 5.1 Add `--dilution` flag to `Bench` command in `cli/src/main.rs`
- [x] 5.2 When `--dilution` is passed, print usage instructions pointing to the Python bench entry point
- [x] 5.3 Add `__main__.py` entry point or CLI argument to `bench/routing.py` so `python -m bench.routing --dilution` invokes `run_dilution_ablation`

## 6. Validation

- [x] 6.1 Run the ablation on an existing query file (fellowship.yaml or glamdring.yaml) against a built repo and verify TSV output contains all three conditions
- [x] 6.2 Verify that all three conditions produce identical files_reached lists (shared descent traces)
- [x] 6.3 Spot-check that log_dilution_D and ratio_dilution_D values are non-negative and vary across control grid settings

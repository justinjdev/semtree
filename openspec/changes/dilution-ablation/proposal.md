## Why

The SRT hyperparameter model (docs/srt_hyperparam_model.tex) proposes two dilution penalty models -- log-dilution D = sum w_l * log(1 + n_l) and ratio model D' = sum w_l * rho_l -- but does not validate which (if either) improves retrieval quality over a pure cost-constraint baseline (mu=0). Over-expansion does not just waste tokens; it actively degrades downstream ranking by forcing the LLM to discriminate among a larger candidate set. We need empirical evidence to choose between these models and calibrate the penalty weight mu.

## What Changes

- Add per-level routing telemetry to track n_l (beam size) and rho_l (irrelevant fraction) during descent
- Implement log-dilution and ratio dilution penalty computation from telemetry data
- Add a `--dilution` flag to the bench infrastructure to run the three-way ablation: no penalty (mu=0), log-dilution, ratio model
- Extend the routing benchmark to compute precision, recall, and MRR alongside existing NDCG@10
- Produce per-condition comparison results in the existing TSV format for analysis

## Capabilities

### New Capabilities
- `dilution-ablation`: Dilution penalty ablation experiment comparing no-penalty, log-dilution, and ratio-dilution models across beam widths, with per-level telemetry (n_l, rho_l) and retrieval metrics (precision, recall, MRR, NDCG)

### Modified Capabilities
- `cli`: Add `--dilution` flag to `bench` subcommand and extend routing telemetry output

## Impact

- `bench/routing.py`: Extended `simulate_descent` to track and return per-level beam sizes and irrelevant fractions
- `bench/harness.py`: New metrics (precision, recall, MRR) added to MetricRecord outputs
- `cli/src/bench.rs`: New dilution ablation phase option
- `cli/src/main.rs`: New `--dilution` flag on `Bench` command
- `bench/analysis.py`: New comparison analysis functions for dilution conditions
- No external dependencies added

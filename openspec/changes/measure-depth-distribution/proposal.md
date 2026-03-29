## Why

The hyperparameter optimization model (docs/srt_hyperparam_model.tex) defines f_k as the probability that a relevant item lies at depth k in the repository tree. The paper explicitly calls out that "estimating the support and shape of f_k on real repositories is a key empirical prerequisite" -- if f_k is concentrated on a narrow band of depths, optimal max_depth is essentially determined and the remaining optimization reduces to beam allocation under a budget. We currently have no tooling to measure f_k, so the theoretical model cannot be validated or parameterized.

## What Changes

- Add a `semtree bench depth-profile` subcommand that computes f_k from benchmark query sets against real repositories
- For each query, determine the depth of each relevant file in the repository tree and weight by graded relevance
- Aggregate across queries and repos to produce f_k distribution statistics
- Output: histogram, mean depth, standard deviation, concentration metrics (entropy, support width), and per-repo/per-category breakdowns
- Add a Python analysis companion (`bench/analysis.py` extension or new `bench/depth_profile.py`) that reads the TSV results and produces summary statistics / plots
- New query file for turborepo (bench/queries/turborepo.yaml) to expand corpus coverage beyond the single fellowship repo

## Capabilities

### New Capabilities
- `depth-profiling`: Measurement and analysis of f_k depth distribution from benchmark query sets against SRT-indexed repositories

### Modified Capabilities
- `cli`: New `bench depth-profile` subcommand added to the CLI command tree

## Impact

- **Code**: `cli/src/bench.rs` (new depth-profile logic), `cli/src/main.rs` (new subcommand variant), `bench/` Python analysis scripts
- **Data**: New TSV metrics with phase="depth-profile" flowing through the existing harness
- **Dependencies**: No new external dependencies; uses existing walkdir, .sem/ record reading, and bench harness infrastructure
- **Specs**: Feeds empirical parameters into the hyperparam optimization model; results directly inform the paper's Section 2 (Repository Model)

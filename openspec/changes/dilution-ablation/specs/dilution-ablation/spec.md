## ADDED Requirements

### Requirement: Per-level routing telemetry
The routing simulation SHALL record per-level telemetry for each descent, capturing: depth index, number of candidate children (n_candidates), number selected (n_selected), the selected paths, and the irrelevant fraction (rho_l) computed against ground truth.

#### Scenario: Telemetry captured during descent
- **WHEN** `simulate_descent` completes a routing simulation for a query
- **THEN** the result SHALL include a list of `LevelTelemetry` records, one per level visited, each containing depth, n_candidates, n_selected, selected_paths, and rho_l

#### Scenario: Irrelevant fraction computation
- **WHEN** a level has 3 selected paths and 1 is on a path to a relevant leaf
- **THEN** rho_l for that level SHALL be 2/3 (0.667)

#### Scenario: All selected are relevant
- **WHEN** all selected paths at a level are on paths to relevant leaves
- **THEN** rho_l for that level SHALL be 0.0

### Requirement: Log-dilution penalty computation
The system SHALL compute the log-dilution penalty D = sum(w_l * log(1 + n_l)) from per-level telemetry, where n_l is n_selected at level l and w_l are configurable level weights defaulting to 1.0.

#### Scenario: Uniform weights with known beam sizes
- **WHEN** a descent visits 3 levels with n_selected = [3, 2, 5] and w_l = [1.0, 1.0, 1.0]
- **THEN** D SHALL equal log(4) + log(3) + log(6) (approximately 4.28)

#### Scenario: Zero beam at a level
- **WHEN** n_selected = 0 at a level
- **THEN** the term for that level SHALL be w_l * log(1) = 0.0

### Requirement: Ratio dilution penalty computation
The system SHALL compute the ratio dilution penalty D' = sum(w_l * rho_l) from per-level telemetry, where rho_l is the irrelevant fraction at level l and w_l are configurable level weights defaulting to 1.0.

#### Scenario: Known irrelevant fractions
- **WHEN** a descent visits 2 levels with rho_l = [0.5, 0.25] and w_l = [1.0, 1.0]
- **THEN** D' SHALL equal 0.75

#### Scenario: Perfect routing (no dilution)
- **WHEN** rho_l = 0.0 at all levels
- **THEN** D' SHALL equal 0.0

### Requirement: Retrieval precision metric
The system SHALL compute precision as the fraction of retrieved files that are in the relevant set.

#### Scenario: 5 files retrieved, 3 relevant
- **WHEN** 5 files are reached and 3 are in the relevant set
- **THEN** precision SHALL be 0.6

#### Scenario: No files retrieved
- **WHEN** 0 files are reached
- **THEN** precision SHALL be 0.0

### Requirement: Retrieval recall metric
The system SHALL compute recall as the fraction of relevant files that were retrieved.

#### Scenario: 10 relevant files, 4 retrieved
- **WHEN** 10 files are in the relevant set and 4 are retrieved
- **THEN** recall SHALL be 0.4

#### Scenario: No relevant files defined
- **WHEN** the relevant set is empty
- **THEN** recall SHALL be 0.0

### Requirement: Mean Reciprocal Rank metric
The system SHALL compute MRR as 1/rank of the first relevant file in the retrieved list.

#### Scenario: First relevant at position 3
- **WHEN** the retrieved list has the first relevant file at position 3 (1-indexed)
- **THEN** MRR SHALL be 1/3 (approximately 0.333)

#### Scenario: No relevant files retrieved
- **WHEN** no relevant file appears in the retrieved list
- **THEN** MRR SHALL be 0.0

### Requirement: Three-way ablation experiment
The system SHALL run the ablation comparing three conditions: (a) no dilution penalty (mu=0), (b) log-dilution penalty, (c) ratio dilution penalty. All three conditions SHALL share the same descent traces and differ only in the penalty computation.

#### Scenario: Ablation produces results for all three conditions
- **WHEN** the dilution ablation runs for a query set
- **THEN** results SHALL contain metric rows for each of the three conditions (no_penalty, log_dilution, ratio_dilution) across all (query, control) combinations

#### Scenario: Shared descent traces
- **WHEN** the ablation runs for a single (query, control) pair
- **THEN** `simulate_descent` SHALL be called exactly once, with penalty scores computed post-hoc from the same telemetry

### Requirement: Ablation results in TSV format
The ablation results SHALL be written to the existing TSV format with system field encoding the dilution condition (e.g., "srt/no_penalty", "srt/log_dilution", "srt/ratio_dilution").

#### Scenario: TSV output format
- **WHEN** ablation results are written
- **THEN** each row SHALL have columns: timestamp, phase, repo, system, query_id, control_json, metric, value -- matching the existing `MetricRecord` schema

#### Scenario: System field encodes condition
- **WHEN** a log-dilution result is written
- **THEN** the system field SHALL be "srt/log_dilution"

### Requirement: Dilution ablation bench entry point
The bench infrastructure SHALL provide a function `run_dilution_ablation` that accepts a repo path, query file, select function, and optional results path, and runs the full ablation experiment.

#### Scenario: Invocation with default parameters
- **WHEN** `run_dilution_ablation(repo_path, query_file, select_fn)` is called
- **THEN** the function SHALL run the control grid with all three dilution conditions and return a list of MetricRecords

#### Scenario: Incremental TSV output
- **WHEN** a results_path is provided
- **THEN** results SHALL be appended to the TSV file incrementally after each (query, control) combination

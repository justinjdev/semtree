## ADDED Requirements

### Requirement: Query set loading
The routing phase SHALL load query sets from YAML files in the `bench/queries/` directory. Each query entry MUST contain a `question` string and an `expected_files` list of repo-relative paths.

#### Scenario: Valid query set loaded
- **WHEN** the routing phase reads `bench/queries/fellowship.yaml` containing 15 queries each with a question and expected files
- **THEN** all 15 queries are loaded and available for evaluation

#### Scenario: Missing query set
- **WHEN** the routing phase cannot find a query set file for the benchmark repo
- **THEN** the phase exits with an error indicating no query set is available for that repo

#### Scenario: Malformed query entry skipped
- **WHEN** a query entry is missing the `expected_files` field
- **THEN** the entry is skipped with a warning and remaining queries are still evaluated

### Requirement: Simulated agent descent
The routing phase SHALL simulate the SRT routing protocol for each query: read `__dir__.md` at the root, use an LLM call to select relevant children based on the query, descend into selected children recursively, and record which leaf files are reached.

#### Scenario: Descent reaches expected files
- **WHEN** the simulated descent for a query about "quest state machine" selects the `cli/internal/state` subtree
- **THEN** the files under that subtree are included in the reached-files set

#### Scenario: Descent stops at leaves
- **WHEN** the descent reaches a file-level `.sem/` record (not a directory)
- **THEN** the file is added to the reached set and no further descent occurs for that branch

#### Scenario: LLM selection at each level
- **WHEN** the descent enters a directory with 8 children
- **THEN** an LLM call is made with the query and the children's descriptions from the routing table, and the LLM selects a subset of children to descend into

### Requirement: Recall at k measurement
The routing phase SHALL compute `recall@k` for each query: the fraction of expected target files that appear in the top-k files reached by the simulated descent. The default k SHALL be 5.

#### Scenario: Perfect recall
- **WHEN** a query has 3 expected files and the simulated descent reaches all 3 within the top 5 files
- **THEN** the `recall@5` for that query is `1.0`

#### Scenario: Partial recall
- **WHEN** a query has 4 expected files and the descent reaches 2 of them in the top 5
- **THEN** the `recall@5` for that query is `0.5`

#### Scenario: Aggregate recall reported
- **WHEN** the routing phase completes all queries
- **THEN** a single `recall@5` metric is reported as the mean across all queries

### Requirement: Reproducible LLM routing calls
The routing phase SHALL use `temperature=0` for all LLM calls during simulated descent to maximize reproducibility across runs.

#### Scenario: Deterministic selection
- **WHEN** the routing phase runs the same query twice against the same SRT
- **THEN** the LLM selects the same children at each routing decision point

### Requirement: Files-opened count
The routing phase SHALL record the total number of `.sem/` records read during simulated descent for each query, reported as a `files_opened` metric. This measures the navigation cost of SRT-guided routing.

#### Scenario: Files opened tracked
- **WHEN** the simulated descent reads 12 `.sem/` records across 3 levels to answer a query
- **THEN** the `files_opened` metric for that query is `12`

#### Scenario: Aggregate files opened
- **WHEN** the routing phase completes all queries
- **THEN** a `mean_files_opened` metric is reported as the average across all queries

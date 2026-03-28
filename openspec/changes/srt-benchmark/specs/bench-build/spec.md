## ADDED Requirements

### Requirement: Full build measurement
The build phase SHALL perform a full (non-incremental) build of the SRT on the benchmark repo and record wall-clock time, total LLM call count, and total node count as metrics.

#### Scenario: Full build metrics collected
- **WHEN** the build phase runs against a repo with 160 files across 30 directories
- **THEN** the results include `build_time_s`, `llm_calls`, and `node_count` metrics with correct values

#### Scenario: Clean state before full build
- **WHEN** the build phase starts a full build
- **THEN** any existing `.sem/` directories in the benchmark repo are removed before building, ensuring a from-scratch measurement

### Requirement: Incremental build measurement
The build phase SHALL also perform an incremental rebuild (no changes) immediately after the full build and record the same metrics, allowing comparison of full vs. no-op incremental cost.

#### Scenario: Incremental no-op build
- **WHEN** the incremental rebuild runs immediately after a full build with no file changes
- **THEN** the `llm_calls` metric is `0` and `build_time_s` is significantly less than the full build time

#### Scenario: Incremental metrics labeled distinctly
- **WHEN** the incremental rebuild metrics are recorded
- **THEN** they use metric names prefixed with `incr_` (e.g., `incr_build_time_s`, `incr_llm_calls`) to distinguish from full build metrics

### Requirement: LLM call counting
The build phase SHALL count the exact number of LLM API calls made during each build. The count MUST include both file summarization and directory summarization calls.

#### Scenario: Call count matches node count on full build
- **WHEN** a full build processes a repo with 160 files and 30 directories
- **THEN** the `llm_calls` metric equals the total number of summarized nodes (files + directories)

#### Scenario: No calls on clean incremental
- **WHEN** an incremental build runs with no changes
- **THEN** the `llm_calls` metric is `0`

### Requirement: Node count reporting
The build phase SHALL report the total number of nodes (files + directories) in the SRT as a metric, providing a measure of repo scale.

#### Scenario: Node count reflects tree size
- **WHEN** the build phase completes on a repo with 160 files and 30 directories
- **THEN** the `node_count` metric equals `190`

### Requirement: Build phase uses force flag for full build
The build phase SHALL use the indexer's `--force` equivalent (bypassing hash checks) for the full build measurement to ensure every node is summarized regardless of prior state.

#### Scenario: Force flag ensures complete rebuild
- **WHEN** the full build runs and stale `.sem/` records exist from a previous run
- **THEN** all nodes are re-summarized, not just those with hash mismatches

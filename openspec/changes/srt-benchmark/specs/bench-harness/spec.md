## ADDED Requirements

### Requirement: CLI bench command
The system SHALL expose a `semtree bench <phase>` command that runs one or more benchmark phases. Valid phases are `build`, `quality`, `routing`, `incremental`, and `all`.

#### Scenario: Run a single phase
- **WHEN** the user runs `semtree bench build`
- **THEN** only the build phase executes against the default benchmark repo

#### Scenario: Run all phases
- **WHEN** the user runs `semtree bench all`
- **THEN** all four phases (build, quality, routing, incremental) execute in sequence

#### Scenario: Invalid phase name
- **WHEN** the user runs `semtree bench foobar`
- **THEN** the command exits with an error listing valid phase names

### Requirement: Repo selection flag
The CLI SHALL accept a `--repo <name>` flag to select which benchmark repo to run against. If omitted, the harness SHALL use the default repo (fellowship).

#### Scenario: Explicit repo selection
- **WHEN** the user runs `semtree bench build --repo fellowship`
- **THEN** the build phase runs against the fellowship benchmark repo

#### Scenario: Unknown repo name
- **WHEN** the user runs `semtree bench build --repo nonexistent`
- **THEN** the command exits with an error indicating the repo is not configured

### Requirement: Phase runner orchestration
The harness SHALL call semtree library functions directly (not via subprocess) to execute each phase. Each phase function SHALL receive the path to the benchmark repo and return a list of metric records.

#### Scenario: Direct library invocation
- **WHEN** the build phase executes
- **THEN** it imports and calls `semtree.builder` functions directly, not through `subprocess` or shell invocation

#### Scenario: Phase returns metrics
- **WHEN** any phase completes successfully
- **THEN** it returns a list of `(metric_name, value, notes)` tuples to the harness

### Requirement: Timing collection
The harness SHALL measure wall-clock time for each phase and include it as a metric in the results.

#### Scenario: Phase duration recorded
- **WHEN** the build phase runs for 45.3 seconds
- **THEN** a metric record with name `phase_time_s` and value `45.3` is included in the results

### Requirement: Results TSV logging
The harness SHALL append all metric records to a `results.tsv` file at the repository root. Each row SHALL contain: `timestamp`, `phase`, `repo`, `metric`, `value`, `notes`. The file SHALL be created with a header row if it does not exist.

#### Scenario: First run creates file with header
- **WHEN** the harness writes results and `results.tsv` does not exist
- **THEN** the file is created with a tab-separated header row followed by the metric rows

#### Scenario: Subsequent runs append
- **WHEN** the harness writes results and `results.tsv` already exists
- **THEN** new metric rows are appended without repeating the header

#### Scenario: TSV format
- **WHEN** the harness records a metric with timestamp `2026-03-28T12:00:00`, phase `build`, repo `fellowship`, metric `build_time_s`, value `180.5`, notes `initial build`
- **THEN** the TSV row contains those six fields separated by tab characters

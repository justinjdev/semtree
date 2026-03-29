## ADDED Requirements

### Requirement: Compute file depth from path
The system SHALL compute the depth of a file as the number of path components in its repo-relative path (e.g., `pkg/teams/types.go` has depth 3, `main.go` has depth 1).

#### Scenario: Nested file depth
- **WHEN** a relevant file has path `pkg/teams/types.go`
- **THEN** its computed depth is 3

#### Scenario: Root-level file depth
- **WHEN** a relevant file has path `main.go`
- **THEN** its computed depth is 1

#### Scenario: Deeply nested file
- **WHEN** a relevant file has path `internal/tui/components/modal/view.go`
- **THEN** its computed depth is 5

### Requirement: Parse query YAML for depth profiling
The system SHALL read benchmark query YAML files (same format as `bench/queries/*.yaml`) and extract each query's relevant file paths and graded relevance scores.

#### Scenario: Standard query file parsed
- **WHEN** the system reads a query YAML containing queries with `relevant` entries having `path` and `relevance` fields
- **THEN** all relevant files and their relevance scores are extracted for depth computation

#### Scenario: Query with no relevant files
- **WHEN** a query entry has no `relevant` field or an empty relevant list
- **THEN** the query is skipped in depth profiling with no error

### Requirement: Relevance-weighted f_k distribution
The system SHALL compute the f_k distribution by weighting each relevant file's contribution to its depth bin by the file's graded relevance score. The distribution SHALL be normalized so that the sum of all f_k values equals 1.

#### Scenario: Weighted distribution computation
- **WHEN** depth 3 has two files with relevance 3 and 1, and depth 2 has one file with relevance 2
- **THEN** f_3 = 4/6, f_2 = 2/6 (total relevance-weighted mass at each depth divided by total)

#### Scenario: Single-depth concentration
- **WHEN** all relevant files across all queries are at depth 4
- **THEN** f_4 = 1.0 and all other f_k = 0.0

### Requirement: Unweighted f_k distribution
The system SHALL also compute an unweighted f_k distribution where each relevant file contributes equally (weight 1) regardless of its relevance score.

#### Scenario: Unweighted vs weighted differ
- **WHEN** depth 2 has one file with relevance 1 and depth 5 has one file with relevance 3
- **THEN** unweighted f_2 = 0.5, f_5 = 0.5 but weighted f_2 = 0.25, f_5 = 0.75

### Requirement: Summary statistics
The system SHALL compute and output the following summary statistics from the f_k distribution:
- Mean depth (relevance-weighted)
- Standard deviation of depth (relevance-weighted)
- Shannon entropy of the f_k distribution
- Support range: minimum and maximum depth with nonzero f_k
- Number of queries analyzed
- Total relevant files counted

#### Scenario: Statistics for concentrated distribution
- **WHEN** f_k is concentrated at depths 3-5 with f_3=0.2, f_4=0.6, f_5=0.2
- **THEN** mean is approximately 4.0, entropy is less than log2(3), and support range is [3, 5]

#### Scenario: Statistics for uniform distribution
- **WHEN** f_k is uniform across depths 1-8
- **THEN** entropy equals log2(8) = 3.0 and support range is [1, 8]

### Requirement: Per-query depth metrics
The system SHALL emit per-query depth metrics including the mean, min, and max depth of relevant files for each query.

#### Scenario: Per-query metrics emitted
- **WHEN** query q01 has relevant files at depths 2, 3, and 3
- **THEN** the output includes q01 with depth_mean ~2.67, depth_min=2, depth_max=3

### Requirement: Repository tree structure metrics
The system SHALL walk the repository directory tree and compute:
- Maximum depth H (deepest file in the repo)
- Branching factor by level B_l (mean number of children per directory at each depth level)

#### Scenario: Tree metrics for a repo
- **WHEN** the repo has files up to depth 6 and the root has 5 subdirectories
- **THEN** repo_max_depth=6 and B_1 is reported as 5 (or the mean fanout at level 1)

### Requirement: TSV output format
The system SHALL output results as TSV rows compatible with the existing benchmark harness format: `(timestamp, phase, repo, system, query_id, control_json, metric, value)`. The phase SHALL be `"depth-profile"`.

#### Scenario: TSV rows written
- **WHEN** depth profiling completes for a query file against a repo
- **THEN** the output TSV contains rows with phase="depth-profile" and metrics including fk_d1, fk_d2, ..., fk_mean, fk_std, fk_entropy

#### Scenario: Append to existing results file
- **WHEN** the results TSV already exists with prior benchmark data
- **THEN** depth-profile rows are appended without duplicating the header

### Requirement: Human-readable summary output
The system SHALL print a human-readable summary to stderr including the f_k histogram, mean depth, standard deviation, entropy, and support range.

#### Scenario: Summary printed to stderr
- **WHEN** depth profiling completes
- **THEN** stderr shows a text histogram of f_k and the computed statistics

### Requirement: Multi-repo aggregation
The system SHALL support running depth profiling across multiple repos (each with their own query file) and computing an aggregate f_k distribution. Per-repo distributions SHALL be normalized independently before averaging to prevent large repos from dominating.

#### Scenario: Two repos aggregated
- **WHEN** repo A has f_3=1.0 and repo B has f_5=1.0
- **THEN** the aggregate distribution has f_3=0.5 and f_5=0.5

### Requirement: Category breakdown
The system SHALL support breaking down f_k by query category (e.g., "focused", "module", "cross-cutting") when the query YAML includes a `category` field.

#### Scenario: Category breakdown reported
- **WHEN** queries have categories "focused" and "cross-cutting"
- **THEN** separate f_k distributions are reported for each category in addition to the overall distribution

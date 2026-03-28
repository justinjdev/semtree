## ADDED Requirements

### Requirement: File modification before rebuild
The incremental phase SHALL modify a known set of files in the benchmark repo before triggering an incremental rebuild. The modifications MUST be deterministic (e.g., appending a fixed comment line) to ensure reproducibility.

#### Scenario: Files modified deterministically
- **WHEN** the incremental phase prepares the benchmark repo
- **THEN** it modifies exactly the configured set of files (e.g., 3 files in distinct subtrees) by appending a known marker line

#### Scenario: Modifications are reversible
- **WHEN** the incremental phase completes
- **THEN** the benchmark repo is restored to its original state (via `git checkout`) so subsequent runs start clean

### Requirement: Changed subtree verification
The incremental phase SHALL verify that only nodes in the changed subtree are re-summarized during the rebuild. Nodes outside the changed subtree MUST retain their original `.sem/` records with unchanged content hashes.

#### Scenario: Changed file re-summarized
- **WHEN** file `src/foo.go` is modified and the incremental rebuild runs
- **THEN** the `.sem/` record for `src/foo.go` has an updated content hash and a new summary

#### Scenario: Unchanged sibling not re-summarized
- **WHEN** file `src/foo.go` is modified but `src/bar.go` is not
- **THEN** the `.sem/` record for `src/bar.go` retains its original content hash

#### Scenario: Ancestor directories re-summarized
- **WHEN** file `src/internal/state.go` is modified
- **THEN** the directory records for `src/internal/` and `src/` are re-summarized (their child hashes changed) but sibling directories are not

### Requirement: Rebuild time measurement
The incremental phase SHALL measure the wall-clock time of the incremental rebuild and report it as the `incr_rebuild_time_s` metric.

#### Scenario: Rebuild time recorded
- **WHEN** the incremental rebuild completes in 8.2 seconds
- **THEN** the `incr_rebuild_time_s` metric has value `8.2`

#### Scenario: Rebuild faster than full build
- **WHEN** the incremental rebuild processes 3 changed files out of 160 total
- **THEN** the `incr_rebuild_time_s` is expected to be substantially less than a full build time

### Requirement: Re-summarized node count
The incremental phase SHALL count the number of nodes that were re-summarized during the rebuild and report it as the `nodes_resummarized` metric. This count MUST include both the directly modified files and any ancestor directories whose hashes changed.

#### Scenario: Count includes ancestors
- **WHEN** 2 files are modified in 2 different subtrees sharing a common parent
- **THEN** `nodes_resummarized` includes the 2 files plus all ancestor directories up to the root

#### Scenario: Count excludes unchanged nodes
- **WHEN** the incremental rebuild completes
- **THEN** `nodes_resummarized` is strictly less than the total `node_count` (assuming not all files were modified)

### Requirement: Correctness after incremental rebuild
The incremental phase SHALL verify that the rebuilt `.sem/` records are correct by recomputing content hashes for all nodes and confirming they match the stored hashes. This ensures the incremental rebuild produces the same result as a full rebuild would.

#### Scenario: Hashes consistent after rebuild
- **WHEN** the incremental rebuild completes
- **THEN** every node's stored content hash matches its freshly recomputed hash

#### Scenario: Equivalent to full rebuild
- **WHEN** the incremental phase optionally runs a full rebuild after the incremental one
- **THEN** all content hashes from the full rebuild match those from the incremental rebuild

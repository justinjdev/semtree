## ADDED Requirements

### Requirement: Children coverage check
The quality phase SHALL verify that every immediate child of a directory appears in its `__dir__.md` routing table under the `## Children` section. The phase SHALL report a `children_coverage` metric as the fraction of children found.

#### Scenario: All children present
- **WHEN** the quality phase checks a directory with 5 children and all 5 appear in the routing table
- **THEN** the `children_coverage` metric for that directory is `1.0`

#### Scenario: Missing children reported
- **WHEN** the quality phase checks a directory with 10 children and 8 appear in the routing table
- **THEN** the `children_coverage` metric is `0.8` and the notes field lists the 2 missing children

#### Scenario: Aggregate coverage metric
- **WHEN** the quality phase completes for the entire repo
- **THEN** a single `children_coverage` metric is reported as the average across all directories

### Requirement: Frontmatter validity check
The quality phase SHALL validate that every `.sem/` record contains YAML frontmatter with the required fields: `path`, `type`, and `content_hash`. The `type` field MUST be either `file` or `directory`.

#### Scenario: Valid frontmatter passes
- **WHEN** a record has frontmatter with `path: src/main.py`, `type: file`, `content_hash: abc123`
- **THEN** the record passes the frontmatter validity check

#### Scenario: Missing field flagged
- **WHEN** a record has frontmatter missing the `content_hash` field
- **THEN** the record is flagged as invalid and included in the `frontmatter_errors` count metric

#### Scenario: Invalid type value flagged
- **WHEN** a file record has `type: folder` instead of `type: file` or `type: directory`
- **THEN** the record is flagged as invalid

### Requirement: Hash consistency check
The quality phase SHALL recompute the content hash for each node and compare it against the hash stored in the `.sem/` record. Any mismatch indicates a stale record.

#### Scenario: Hashes match
- **WHEN** the recomputed hash for a file matches the `content_hash` in its `.sem/` record
- **THEN** the file passes the hash consistency check

#### Scenario: Stale hash detected
- **WHEN** the recomputed hash differs from the stored `content_hash`
- **THEN** the record is flagged as stale and included in the `stale_records` count metric

### Requirement: No orphan records
The quality phase SHALL detect orphan `.sem/` records -- records whose corresponding source file or directory no longer exists in the repo. The phase SHALL report an `orphan_records` count metric.

#### Scenario: No orphans in clean build
- **WHEN** the quality phase runs immediately after a fresh full build
- **THEN** the `orphan_records` metric is `0`

#### Scenario: Orphan detected after file deletion
- **WHEN** a source file has been deleted but its `.sem/` record still exists
- **THEN** the record is counted as an orphan and the notes field lists its path

### Requirement: Deterministic rebuild check
The quality phase SHALL verify that building the SRT twice on the same repo content produces identical content hashes for all nodes. This confirms the build process is deterministic.

#### Scenario: Two builds produce same hashes
- **WHEN** the quality phase builds the SRT, records all hashes, rebuilds with `--force`, and records hashes again
- **THEN** every node's content hash is identical across both builds

#### Scenario: Non-determinism detected
- **WHEN** any node's hash differs between the two builds
- **THEN** the `determinism_failures` metric is non-zero and the notes field lists the affected paths

## ADDED Requirements

### Requirement: Hash-based freshness check
The indexer SHALL compare each node's freshly computed content hash against the hash stored in its existing `.sem/` record. If the hashes match, the node SHALL be skipped (no LLM call, no record rewrite).

#### Scenario: Unchanged file skipped
- **WHEN** the indexer processes a file whose content hash matches the hash in its existing `.sem/` record
- **THEN** no LLM call is made and the existing record is preserved

#### Scenario: Changed file re-summarized
- **WHEN** the indexer processes a file whose content hash differs from the stored hash
- **THEN** a new LLM summary is generated and the record is overwritten with the new hash and summary

#### Scenario: Directory re-summarized when child changes
- **WHEN** any child of a directory has been re-summarized (hash changed)
- **THEN** the directory's hash changes and its summary is regenerated

### Requirement: Missing record treated as stale
If a node has no existing `.sem/` record, the indexer SHALL treat it as needing summarization (equivalent to a hash mismatch).

#### Scenario: New file gets summarized
- **WHEN** the indexer encounters a file with no existing `.sem/` record
- **THEN** the file is summarized and a new record is created

### Requirement: Crash resumability
Because each record is written independently and the freshness check is hash-based, a build that is interrupted partway through SHALL be resumable by re-running the same build command. Nodes that were successfully written before the interruption will pass the freshness check and be skipped.

#### Scenario: Interrupted build resumed
- **WHEN** a build is interrupted after processing 50 of 100 files, and the build command is re-run
- **THEN** the 50 already-processed files are skipped (hashes match) and only the remaining 50 are summarized

### Requirement: Force rebuild flag
The indexer SHALL support a `--force` flag that skips all freshness checks and re-summarizes every node regardless of hash match.

#### Scenario: Force rebuild regenerates all
- **WHEN** the indexer is run with `--force` on a repository with all records up-to-date
- **THEN** every node is re-summarized and every record is rewritten

## ADDED Requirements

### Requirement: File content hashing
The indexer SHALL compute a SHA-256 hash of a file's raw byte contents. The hash SHALL be stored as a lowercase hexadecimal string.

#### Scenario: File hash computed from contents
- **WHEN** the indexer processes a file with known contents
- **THEN** the stored `content_hash` equals the SHA-256 hex digest of those exact bytes

#### Scenario: Identical files produce identical hashes
- **WHEN** two files have identical byte contents
- **THEN** their `content_hash` values are identical

### Requirement: Directory content hashing
The indexer SHALL compute a directory's hash from its immediate children. The hash is the SHA-256 of a canonical string formed by: sorting all immediate child `(repo_relative_path, child_hash)` pairs lexicographically by path, formatting each as `path:hash`, joining with newlines, and hashing the resulting UTF-8 bytes.

#### Scenario: Directory hash changes when a child file changes
- **WHEN** a file within a directory is modified (changing its content hash)
- **THEN** the directory's `content_hash` changes on the next build

#### Scenario: Directory hash is independent of processing order
- **WHEN** the same directory is processed with children discovered in different filesystem orders
- **THEN** the resulting `content_hash` is identical both times

#### Scenario: Directory hash propagates upward
- **WHEN** a deeply nested file changes
- **THEN** every ancestor directory's `content_hash` changes

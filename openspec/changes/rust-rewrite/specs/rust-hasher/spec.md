## ADDED Requirements

### Requirement: File content hashing
The hasher SHALL compute a SHA-256 hash of a file's raw byte contents. The hash SHALL be represented as a lowercase hexadecimal string (64 characters).

#### Scenario: File hash computed from contents
- **WHEN** the hasher processes a file with known contents
- **THEN** the returned hash equals the SHA-256 hex digest of those exact bytes

#### Scenario: Identical files produce identical hashes
- **WHEN** two files have identical byte contents
- **THEN** their computed hashes are identical

#### Scenario: Different files produce different hashes
- **WHEN** two files differ by even a single byte
- **THEN** their computed hashes are different

### Requirement: Directory content hashing
The hasher SHALL compute a directory's hash from its immediate children. The hash is the SHA-256 of a canonical string formed by: sorting all immediate child `(repo_relative_path, child_hash)` pairs lexicographically by path, formatting each as `"path:hash\n"`, concatenating all pairs, and hashing the resulting UTF-8 bytes.

#### Scenario: Directory hash computed from children
- **WHEN** a directory has children `b.rs` (hash `bbb`) and `a.rs` (hash `aaa`)
- **THEN** the directory hash is SHA-256 of `"a.rs:aaa\nb.rs:bbb\n"`

#### Scenario: Directory hash is independent of processing order
- **WHEN** the same directory is processed with children discovered in different filesystem orders
- **THEN** the resulting hash is identical both times

### Requirement: Deterministic hashing
The hasher SHALL produce identical hashes for identical content across platforms and invocations. The same file bytes MUST always produce the same hash. The same set of children MUST always produce the same directory hash.

#### Scenario: Reproducible across invocations
- **WHEN** the hasher processes the same file twice without modification
- **THEN** the hash is identical both times

#### Scenario: Reproducible across platforms
- **WHEN** the same file contents are hashed on macOS and Linux
- **THEN** the hash is identical on both platforms

### Requirement: Hash propagation through ancestors
Changing a leaf file's content SHALL cause all ancestor directory hashes to change, since each directory hash depends on its children's hashes.

#### Scenario: Single file change propagates to root
- **WHEN** a file `src/auth/login.rs` is modified
- **THEN** the hashes for `src/auth/`, `src/`, and the root directory all change on the next computation

#### Scenario: Unchanged sibling preserves its hash
- **WHEN** a file `src/auth/login.rs` is modified but `src/auth/session.rs` is not
- **THEN** `session.rs` retains its original hash while the parent directory hash changes

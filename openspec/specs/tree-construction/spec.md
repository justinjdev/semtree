## ADDED Requirements

### Requirement: Post-order DFS traversal
The indexer SHALL traverse the target repository using post-order depth-first search, processing all children of a directory before processing the directory itself. Within each directory, entries SHALL be sorted lexicographically for deterministic ordering.

#### Scenario: Children processed before parent
- **WHEN** the indexer processes a directory containing files and subdirectories
- **THEN** all files and subdirectories within it are fully processed before the directory's own record is created

#### Scenario: Deterministic ordering
- **WHEN** the indexer traverses a directory containing files `c.py`, `a.py`, `b.py`
- **THEN** the files are processed in order `a.py`, `b.py`, `c.py`

### Requirement: Dotfile and dot-directory exclusion
The indexer SHALL skip all files and directories whose names start with `.` (dot). This includes `.git/`, `.env`, `.srt/`, `.sem/`, and any other dot-prefixed entries.

#### Scenario: Dot-directory skipped
- **WHEN** the indexer encounters a directory named `.git`
- **THEN** the directory and all its contents are skipped entirely

#### Scenario: Dotfile skipped
- **WHEN** the indexer encounters a file named `.env`
- **THEN** the file is not processed and no `.sem/` record is created for it

### Requirement: Symlink exclusion
The indexer SHALL skip all symbolic links, whether they point to files or directories.

#### Scenario: Symlink skipped
- **WHEN** the indexer encounters a symlink `link.py` pointing to `../other/real.py`
- **THEN** the symlink is not processed and no `.sem/` record is created for it

### Requirement: Binary file exclusion
The indexer SHALL skip binary files. A file is considered binary if reading the first 8192 bytes reveals one or more null bytes (`\x00`).

#### Scenario: Binary file detected and skipped
- **WHEN** the indexer encounters a file containing null bytes in its first 8192 bytes
- **THEN** the file is not processed and no `.sem/` record is created for it

#### Scenario: Text file with no null bytes is included
- **WHEN** the indexer encounters a file with no null bytes in its first 8192 bytes
- **THEN** the file is included in the tree and processed normally

### Requirement: Repository root as tree root
The indexer SHALL use the specified repository path as the root node of the SRT. All paths in the tree SHALL be relative to this root.

#### Scenario: Paths are repo-relative
- **WHEN** the indexer processes `/home/user/myrepo/src/foo.py` with repo root `/home/user/myrepo`
- **THEN** the node's path is recorded as `src/foo.py`

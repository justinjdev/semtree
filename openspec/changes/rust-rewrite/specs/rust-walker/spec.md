## ADDED Requirements

### Requirement: Post-order DFS traversal
The walker SHALL traverse the target directory using post-order depth-first search, processing all children of a directory before the directory itself. Within each directory, entries SHALL be sorted lexicographically for deterministic ordering.

#### Scenario: Children processed before parent
- **WHEN** the walker traverses a directory containing files and subdirectories
- **THEN** all files and subdirectories are yielded before the directory itself

#### Scenario: Deterministic ordering within a directory
- **WHEN** the walker traverses a directory containing `c.rs`, `a.rs`, `b.rs`
- **THEN** the files are yielded in order `a.rs`, `b.rs`, `c.rs`

### Requirement: Git-aware traversal
The walker SHALL detect whether the target path is inside a git repository. When inside a git repo, the walker SHALL use `git ls-files` to enumerate tracked files, automatically respecting `.gitignore` rules.

#### Scenario: Git repo uses git ls-files
- **WHEN** the walker is invoked on a path inside a git repository
- **THEN** it enumerates files using `git ls-files` and only yields tracked, non-ignored files

#### Scenario: Git ls-files respects .gitignore
- **WHEN** the repository has a `.gitignore` containing `target/` and a `target/` directory exists
- **THEN** no files under `target/` are yielded

### Requirement: Filesystem fallback for non-git repos
The walker SHALL fall back to a recursive filesystem walk when the target path is not inside a git repository.

#### Scenario: Non-git directory uses filesystem walk
- **WHEN** the walker is invoked on a directory that is not inside a git repository
- **THEN** it recursively walks the filesystem to discover files

### Requirement: Binary file exclusion
The walker SHALL skip binary files. A file is considered binary if the first 8192 bytes contain one or more null bytes (`\x00`).

#### Scenario: Binary file detected and skipped
- **WHEN** the walker encounters a compiled binary file containing null bytes in its first 8192 bytes
- **THEN** the file is not yielded

#### Scenario: Text file with no null bytes is included
- **WHEN** the walker encounters a source file with no null bytes in its first 8192 bytes
- **THEN** the file is yielded normally

### Requirement: Dotfile and dot-directory exclusion
The walker SHALL skip all files and directories whose names start with `.` (dot). This includes `.git/`, `.env`, `.sem/`, and any other dot-prefixed entries.

#### Scenario: Dot-directory skipped
- **WHEN** the walker encounters a directory named `.git`
- **THEN** the directory and all its contents are skipped entirely

#### Scenario: Dotfile skipped
- **WHEN** the walker encounters a file named `.env`
- **THEN** the file is not yielded

### Requirement: Symlink exclusion
The walker SHALL skip all symbolic links, whether they point to files or directories.

#### Scenario: Symlink skipped
- **WHEN** the walker encounters a symlink `link.rs` pointing to `../other/real.rs`
- **THEN** the symlink is not yielded

### Requirement: Exclude glob patterns
The walker SHALL accept a list of `--exclude` glob patterns. Any file or directory matching an exclude pattern SHALL be skipped.

#### Scenario: Exclude pattern filters files
- **WHEN** the walker is invoked with `--exclude "*.generated.rs"`
- **THEN** files matching the pattern such as `schema.generated.rs` are not yielded

#### Scenario: Exclude pattern filters directories
- **WHEN** the walker is invoked with `--exclude "vendor/*"`
- **THEN** the `vendor/` directory and all its contents are skipped

#### Scenario: Multiple exclude patterns
- **WHEN** the walker is invoked with `--exclude "*.lock" --exclude "testdata/*"`
- **THEN** both `Cargo.lock` and files under `testdata/` are skipped

### Requirement: Node struct output
The walker SHALL yield Node structs for each discovered entry. Each Node SHALL contain: `repo_relative_path` (relative to the repo root), `absolute_path`, `is_directory` (boolean), and `children` (list of child Nodes for directories, empty for files).

#### Scenario: File node fields
- **WHEN** the walker yields a file node for `/home/user/repo/src/main.rs` with repo root `/home/user/repo`
- **THEN** the node has `repo_relative_path: "src/main.rs"`, `absolute_path: "/home/user/repo/src/main.rs"`, `is_directory: false`, and `children: []`

#### Scenario: Directory node contains children
- **WHEN** the walker yields a directory node for `src/` containing `main.rs` and `lib.rs`
- **THEN** the node has `is_directory: true` and `children` contains the two file nodes

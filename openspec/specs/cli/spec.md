## ADDED Requirements

### Requirement: Build command
The indexer SHALL expose a `build` command that constructs or incrementally updates the SRT for a target repository.

#### Scenario: Build with default path
- **WHEN** the user runs `semtree build` with no path argument
- **THEN** the indexer builds the SRT for the current working directory

#### Scenario: Build with explicit path
- **WHEN** the user runs `semtree build /path/to/repo`
- **THEN** the indexer builds the SRT for the specified directory

#### Scenario: Target path does not exist
- **WHEN** the user runs `semtree build /nonexistent/path`
- **THEN** the indexer exits with a clear error message

### Requirement: Model selection flag
The indexer SHALL accept a `--model` flag to specify which LLM model to use for summarization. The default SHALL be `claude-sonnet-4-20250514`.

#### Scenario: Custom model specified
- **WHEN** the user runs `semtree build --model claude-haiku-4-5-20251001`
- **THEN** the summarization calls use the specified model

#### Scenario: Default model used
- **WHEN** the user runs `semtree build` without `--model`
- **THEN** the summarization calls use `claude-sonnet-4-20250514`

### Requirement: Max tokens flag
The indexer SHALL accept a `--max-tokens` flag to configure the oversized file threshold. The default SHALL be 100000.

#### Scenario: Custom token limit
- **WHEN** the user runs `semtree build --max-tokens 50000`
- **THEN** files estimated at over 50000 tokens are marked as oversized

### Requirement: Force rebuild flag
The indexer SHALL accept a `--force` flag that disables incremental hash checks and rebuilds all records.

#### Scenario: Force flag passed
- **WHEN** the user runs `semtree build --force`
- **THEN** all nodes are re-summarized regardless of hash match

### Requirement: Progress output
The indexer SHALL print progress information to stderr during the build, including the number of nodes processed and skipped.

#### Scenario: Progress displayed during build
- **WHEN** the indexer processes a repository with 10 files, 5 unchanged
- **THEN** stderr output indicates 5 files summarized and 5 files skipped (up-to-date)

### Requirement: Installable CLI entry point
The package SHALL be installable via `pip install .` (or `pip install -e .` for development) and expose a `semtree` command-line entry point.

#### Scenario: CLI available after install
- **WHEN** the user runs `pip install .` in the project root
- **THEN** the `semtree` command is available on PATH

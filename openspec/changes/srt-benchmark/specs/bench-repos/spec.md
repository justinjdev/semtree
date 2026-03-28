## ADDED Requirements

### Requirement: Clone at pinned commit
The repo manager SHALL clone benchmark repos at a specific pinned commit SHA to ensure reproducible benchmarks. The commit SHA MUST be configured per repo, not determined at clone time.

#### Scenario: Repo cloned at exact commit
- **WHEN** the repo manager clones the fellowship repo with pinned SHA `abc1234`
- **THEN** the resulting working tree is checked out at exactly that commit

#### Scenario: Pinned commit not found
- **WHEN** the configured commit SHA does not exist in the remote repository
- **THEN** the clone fails with a clear error indicating the pinned commit is missing

### Requirement: Local caching
The repo manager SHALL cache cloned repos locally to avoid re-cloning on every benchmark run. The cache directory SHALL default to `bench/.repos/` within the semtree project root.

#### Scenario: First run clones
- **WHEN** the benchmark runs for the first time and no cached repo exists
- **THEN** the repo is cloned from the remote and stored in the cache directory

#### Scenario: Subsequent runs use cache
- **WHEN** the benchmark runs and a cached repo at the correct pinned commit already exists
- **THEN** no network fetch occurs and the cached repo is used directly

#### Scenario: Stale cache re-cloned
- **WHEN** the cached repo exists but is at a different commit than the pinned SHA
- **THEN** the cached repo is deleted and re-cloned at the correct commit

### Requirement: Three size tiers
The repo manager SHALL support three size tiers for benchmark repos: `small` (under 200 files), `medium` (200-1000 files), and `large` (over 1000 files). Each tier SHALL have at least one configured repo.

#### Scenario: Small tier repo
- **WHEN** the harness requests a small-tier benchmark repo
- **THEN** the repo manager provides a repo with fewer than 200 source files (e.g., fellowship)

#### Scenario: Tier selection via CLI
- **WHEN** the user runs `semtree bench build --repo fellowship`
- **THEN** the harness loads the fellowship repo configuration including its tier, remote URL, and pinned commit

#### Scenario: Repo config includes tier metadata
- **WHEN** the repo manager loads repo configuration
- **THEN** each repo entry contains `name`, `url`, `commit`, `tier`, and `description` fields

### Requirement: Repo configuration file
The repo manager SHALL read benchmark repo definitions from a configuration file at `bench/repos.yaml`. Each entry MUST specify the repo name, git URL, pinned commit SHA, size tier, and a human-readable description.

#### Scenario: Valid config loaded
- **WHEN** the repo manager reads `bench/repos.yaml` containing 3 repo entries
- **THEN** all 3 repos are available for benchmark selection

#### Scenario: Config file missing
- **WHEN** `bench/repos.yaml` does not exist
- **THEN** the repo manager exits with an error indicating the config file is required

### Requirement: Cleanup command
The repo manager SHALL support a cleanup operation that removes all cached benchmark repos to reclaim disk space. This SHALL be accessible via `semtree bench --clean`.

#### Scenario: Clean removes cached repos
- **WHEN** the user runs `semtree bench --clean`
- **THEN** the `bench/.repos/` directory and all its contents are removed

#### Scenario: Clean on empty cache
- **WHEN** the user runs `semtree bench --clean` and no cached repos exist
- **THEN** the command completes without error

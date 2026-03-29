## ADDED Requirements

### Requirement: CLI binary entry point
The system SHALL produce a single static binary named `semtree` installable via `cargo install` from the `cli/` workspace. The binary SHALL use clap for argument parsing and subcommand dispatch.

#### Scenario: Binary available after cargo install
- **WHEN** the user runs `cargo install --path cli/`
- **THEN** the `semtree` command is available on PATH

#### Scenario: No arguments shows help
- **WHEN** the user runs `semtree` with no arguments
- **THEN** a help message is displayed listing all available subcommands

### Requirement: Build subcommand
The `build` subcommand SHALL walk the repository, compute hashes, summarize changed nodes via LLM, and write `.sem/` records. It SHALL accept an optional positional path argument defaulting to the current working directory.

#### Scenario: Build with default path
- **WHEN** the user runs `semtree build`
- **THEN** the SRT is built for the current working directory

#### Scenario: Build with explicit path
- **WHEN** the user runs `semtree build /path/to/repo`
- **THEN** the SRT is built for the specified directory

#### Scenario: Build target does not exist
- **WHEN** the user runs `semtree build /nonexistent/path`
- **THEN** the command exits with a non-zero status and a clear error message

### Requirement: Build --model flag
The `build` subcommand SHALL accept a `--model` flag to specify the LLM model for summarization. The default SHALL be `claude-sonnet-4-20250514`.

#### Scenario: Custom model for build
- **WHEN** the user runs `semtree build --model claude-haiku-4-5-20251001`
- **THEN** all summarization calls use the specified model

#### Scenario: Default model for build
- **WHEN** the user runs `semtree build` without `--model`
- **THEN** summarization calls use `claude-sonnet-4-20250514`

### Requirement: Build --max-tokens flag
The `build` subcommand SHALL accept a `--max-tokens` flag to configure the oversized file threshold. The default SHALL be 100000.

#### Scenario: Custom max-tokens
- **WHEN** the user runs `semtree build --max-tokens 50000`
- **THEN** files estimated at over 50000 tokens are marked as oversized

### Requirement: Build --force flag
The `build` subcommand SHALL accept a `--force` flag that disables incremental hash checks and rebuilds all records regardless of hash match.

#### Scenario: Force rebuild
- **WHEN** the user runs `semtree build --force`
- **THEN** all nodes are re-summarized even if their content hash matches the existing record

### Requirement: Build --exclude flag
The `build` subcommand SHALL accept one or more `--exclude` glob patterns passed to the walker for filtering.

#### Scenario: Exclude patterns applied during build
- **WHEN** the user runs `semtree build --exclude "*.lock" --exclude "vendor/*"`
- **THEN** matching files and directories are excluded from the build

### Requirement: Build --no-embed flag
The `build` subcommand SHALL accept a `--no-embed` flag that skips the embedding phase after summarization. By default, build SHALL compute embeddings after writing records.

#### Scenario: Build without embeddings
- **WHEN** the user runs `semtree build --no-embed`
- **THEN** summaries are generated and records are written, but no embeddings are computed

#### Scenario: Build with embeddings by default
- **WHEN** the user runs `semtree build` without `--no-embed`
- **THEN** embeddings are computed after records are written

### Requirement: Build --embed-model flag
The `build` subcommand SHALL accept an `--embed-model` flag to specify the embedding model. The default SHALL be `BAAI/bge-small-en-v1.5`.

#### Scenario: Custom embed model
- **WHEN** the user runs `semtree build --embed-model custom/model`
- **THEN** the embedding phase uses the specified model

### Requirement: Embed subcommand
The `embed` subcommand SHALL compute embeddings for existing `.sem/` records without rebuilding summaries. It SHALL accept an optional positional path argument defaulting to the current working directory.

#### Scenario: Embed existing records
- **WHEN** the user runs `semtree embed`
- **THEN** embeddings are computed for all `.sem/` records in the current directory tree

#### Scenario: Embed with --force
- **WHEN** the user runs `semtree embed --force`
- **THEN** all embeddings are recomputed even if the content hash matches existing `.vec` files

#### Scenario: Embed with --model
- **WHEN** the user runs `semtree embed --model custom/model`
- **THEN** the specified embedding model is used

### Requirement: Query subcommand
The `query` subcommand SHALL rank a directory's immediate children by cosine similarity to a query string. It SHALL require a `query` argument and an optional `path` argument defaulting to the current working directory.

#### Scenario: Query ranks children
- **WHEN** the user runs `semtree query "authentication logic" src/`
- **THEN** the immediate children of `src/` are ranked by cosine similarity and printed in descending order

#### Scenario: Query with --top-k
- **WHEN** the user runs `semtree query "auth" --top-k 5`
- **THEN** only the top 5 results are displayed

#### Scenario: Query with --threshold
- **WHEN** the user runs `semtree query "auth" --threshold 0.3`
- **THEN** only results with cosine similarity >= 0.3 are displayed

#### Scenario: Query with --model
- **WHEN** the user runs `semtree query "auth" --model custom/model`
- **THEN** the specified embedding model is used for the query embedding

### Requirement: Route subcommand
The `route` subcommand SHALL perform a full beam-search descent through the SRT to find the most relevant files for a query. It SHALL require a `query` argument and an optional `path` argument.

#### Scenario: Route returns file paths
- **WHEN** the user runs `semtree route "how does authentication work"`
- **THEN** the command outputs a ranked list of file paths most relevant to the query

#### Scenario: Route with --beam-width
- **WHEN** the user runs `semtree route "auth" --beam-width 5`
- **THEN** the beam search considers up to 5 candidates at each level

#### Scenario: Route with --max-depth
- **WHEN** the user runs `semtree route "auth" --max-depth 3`
- **THEN** the beam search descends at most 3 levels deep

#### Scenario: Route with --model
- **WHEN** the user runs `semtree route "auth" --model custom/model`
- **THEN** the specified embedding model is used for routing

### Requirement: Serve subcommand
The `serve` subcommand SHALL start a daemon process listening on a Unix socket. The daemon SHALL keep the embedding model loaded in memory for sub-5ms warm routing.

#### Scenario: Serve starts daemon
- **WHEN** the user runs `semtree serve`
- **THEN** a daemon starts listening on the default socket path `~/.cache/semtree/semtree.sock`

#### Scenario: Serve with --socket
- **WHEN** the user runs `semtree serve --socket /tmp/semtree.sock`
- **THEN** the daemon listens on the specified socket path

#### Scenario: Route detects running daemon
- **WHEN** a daemon is running and the user runs `semtree route "auth"`
- **THEN** the route command sends the request to the daemon instead of loading the model inline

#### Scenario: Route falls back without daemon
- **WHEN** no daemon is running and the user runs `semtree route "auth"`
- **THEN** the route command loads the model inline and performs the routing directly

### Requirement: Bench subcommand
The `bench` subcommand SHALL run benchmark phases for data collection. It SHALL require a `phase` positional argument. Valid phases are `build`, `quality`, `routing`, `incremental`, and `all`.

#### Scenario: Run single benchmark phase
- **WHEN** the user runs `semtree bench build`
- **THEN** only the build benchmark phase executes

#### Scenario: Run all benchmark phases
- **WHEN** the user runs `semtree bench all`
- **THEN** all benchmark phases execute in sequence

#### Scenario: Invalid phase name
- **WHEN** the user runs `semtree bench foobar`
- **THEN** the command exits with an error listing valid phase names

#### Scenario: Bench with --repo
- **WHEN** the user runs `semtree bench build --repo fellowship`
- **THEN** the build phase runs against the named benchmark repo

#### Scenario: Bench with --repo-path
- **WHEN** the user runs `semtree bench build --repo-path /tmp/test-repo`
- **THEN** the build phase runs against the repo at the specified path

#### Scenario: Bench with --results
- **WHEN** the user runs `semtree bench build --results /tmp/results.tsv`
- **THEN** benchmark metrics are written to the specified results file

### Requirement: Progress output
All subcommands that perform multi-step operations (build, embed, bench) SHALL print progress information to stderr, including counts of nodes processed and skipped.

#### Scenario: Build progress displayed
- **WHEN** `semtree build` processes a repository with 10 files, 5 unchanged
- **THEN** stderr output indicates 5 files summarized and 5 files skipped (up-to-date)

# semtree

**Semantic Resolution Trees** — a deterministic summary hierarchy that mirrors a repository's filesystem, stored as colocated Markdown records tracked by git. Embedding-assisted routing enables sub-10ms code navigation.

## What is it?

semtree builds a `.sem/` directory alongside your code containing natural-language summaries of every file and directory. These summaries form a tree that agents (or humans) can traverse top-down to find relevant code without reading everything.

The key insight: **summaries route, code answers.** The tree gets you to the right neighborhood in milliseconds; you read source files to get the actual answer.

```
repo/
├── .sem/
│   ├── __dir__.md          # root summary + routing table
│   ├── src.md              # directory sibling (for embedding ranking)
│   ├── README.md.md        # file summary
│   └── src.vec             # embedding vector (binary)
├── src/
│   ├── .sem/
│   │   ├── __dir__.md
│   │   ├── auth.py.md
│   │   └── auth.py.vec
│   └── auth.py
└── README.md
```

## Install

### From source (Rust)

```bash
cd cli
cargo install --path .
```

Requires Rust 2024 edition. The embedding model (~30MB) downloads automatically on first use.

### Python (for benchmarking only)

```bash
pip install -e '.[dev]'
```

## Quick Start

### 1. Build the SRT

```bash
# Summarize every file and directory using an LLM
semtree build /path/to/repo

# This creates .sem/ directories with Markdown summaries
# and .vec embedding files for routing
```

Requires `claude` CLI on PATH ([Claude Code](https://claude.ai/code)).

### 2. Query a directory

```bash
# Rank children of a directory by relevance to your question
semtree query "how does authentication work?" /path/to/repo
```

Output:
```
0.7842  src/auth/       Authentication and session management
0.6231  src/middleware/  HTTP middleware including auth guards
0.4102  README.md       Project overview and setup instructions
```

### 3. Route through the tree

```bash
# Full top-down descent from root to candidate files
semtree route "how does the quest state machine work?" /path/to/repo
```

Output:
```
--- . (8 children) [0ms] ---
  0.6376  plugin/   Claude Code plugin definitions...
  0.5798  cli/      Go CLI binary for backend operations...

--- cli/internal/ (15 children) [0ms] ---
  0.7075  cli/internal/hooks/   Plugin hook implementations...
  0.6726  cli/internal/state/   Quest state management...

Route complete in 78ms
```

### 4. Start the daemon (sub-10ms routing)

```bash
# Start background daemon — keeps embedding model in memory
semtree serve

# Subsequent route/query calls use the daemon automatically
# Warm routing: ~11ms for a 300-node tree
```

## Commands

| Command | Description |
|---|---|
| `semtree build [path]` | Build/update SRT (summarize + embed) |
| `semtree embed [path]` | Compute embeddings for existing summaries |
| `semtree query <query> [path]` | Rank directory children by similarity |
| `semtree route <query> [path]` | Full beam-search descent to candidate files |
| `semtree search <query> [path]` | Query-adaptive search (classifies intent, routes to best backend) |
| `semtree review [range] [path]` | Generate review manifest for code changes |
| `semtree impact [path]` | Find files related to changed files |
| `semtree serve` | Start daemon for warm routing |
| `semtree bench <phase>` | Run benchmark evaluation (quality, routing, diagnostics) |
| `semtree vec inspect <file>` | Inspect binary .vec file |

### Build options

```bash
semtree build /path/to/repo \
  --model claude-sonnet-4-20250514 \  # LLM for summarization
  --max-tokens 100000 \               # oversized file threshold
  --force \                            # rebuild all (ignore cache)
  --exclude 'vendor/*' \              # glob patterns to skip
  --no-embed \                        # skip embedding step
  --embed-model BAAI/bge-small-en-v1.5 \  # embedding model
  --batch \                            # use Batch API (50% cost savings)
  --verify \                           # BottleSum orphan detection (re-summarize if children are lost)
  --fidelity-threshold 0.3 \          # cosine sim below which a child is orphaned
  --orphan-rate 0.2                    # max orphan fraction before re-summarization
```

### Search (query-adaptive router)

```bash
# Classifies intent and routes to the best backend automatically
semtree search "where is EngineBuilder defined?" /path/to/repo
# → [intent: exact-lookup] — uses ripgrep

semtree search "how does the task graph execution work?" /path/to/repo
# → [intent: semantic-architectural] — uses SRT descent

semtree search "how does auth flow across services?" /path/to/repo
# → [intent: cross-cutting] — uses SRT with wider beam

semtree search "how does retry logic work?" /path/to/repo
# → [intent: mixed] — runs both ripgrep and SRT, merges results
```

Five intent classes (checked by heuristic in priority order): exact-lookup, lexical, cross-cutting, semantic-architectural, mixed. No LLM call for classification.

### Route options

```bash
semtree route "your question" /path/to/repo \
  --beam-width 7 \          # children to explore per level (default: 7)
  --max-depth 5 \            # max descent depth (default: 5)
  --beam-policy uniform \    # uniform or waterfill
  --model BAAI/bge-small-en-v1.5
```

### Review (code review manifest)

```bash
# Generate review manifest for uncommitted changes
semtree review /path/to/repo

# For a specific range (PR, branch)
semtree review "main..HEAD" /path/to/repo
```

Outputs a three-section markdown manifest: severity triage table (from embedding fan-out), per-file semantic context with related files, and cross-cutting warnings for potentially missed changes. No LLM calls — pure embedding math.

## How it works

### Offline build

1. **Walk** the filesystem (git-aware, respects .gitignore)
2. **Hash** each file (SHA-256) and directory (hash of sorted children)
3. **Summarize** via LLM — only files whose hash changed since last build
4. **Write** `.sem/` records (YAML frontmatter + Markdown summary)
5. **Embed** summaries into 384-dim vectors (BAAI/bge-small-en-v1.5)

### Online routing

1. Embed your query once
2. At each directory level, rank children by cosine similarity
3. Select top-k (beam width), descend into directories, collect files
4. Return candidate files sorted by score

No LLM calls at query time — pure vector math.

## Architecture

```
cli/src/
├── main.rs          # CLI entry point (clap)
├── walker.rs        # Git-aware filesystem traversal
├── hasher.rs        # SHA-256 content hashing
├── records.rs       # .sem/ record I/O (YAML + Markdown)
├── summarizer.rs    # LLM summarization (Anthropic API, batch, BottleSum verify)
├── builder.rs       # Build pipeline orchestration
├── embedder.rs      # Embedding inference + cosine ranking + routing
├── vec_store.rs     # Binary .vec format with mmap
├── server.rs        # Unix socket daemon (tokio)
├── search.rs        # Query-adaptive router (5-class intent classification)
├── review.rs        # Review manifest generator (triage, context, warnings)
├── bench.rs         # Benchmark data collection + diagnostics
└── depth_profile.rs # Tree depth and branching analysis
```

### .sem/ record format

```markdown
---
path: src/auth.py
type: file
content_hash: e5f6a7b8c9d0...
---

Authentication module handling user sessions,
JWT validation, and permission guards.
```

### Binary .vec format

16-byte header (magic `SVEC`, version, dims, hash length) followed by content hash, model name, and raw f32 vector. Memory-mapped for zero-copy reads.

## Benchmarks

### Turborepo (40 queries, 59 Rust crates)

| System | Avg NDCG@10 | Queries hit | P50 Latency |
|---|---|---|---|
| **SRT (daemon)** | **0.659** | **38/40** | **13.2ms** |
| Shire (FTS+RAG) | 0.476 | 33/40 | 6.1ms |
| ripgrep | 0.397 | 36/40 | 404ms |
| grep | 0.384 | 38/40 | 1304ms |

SRT leads in every query category (focused 0.824, module 0.625, cross-cutting 0.506). Optimal parameters found via 40-config sweep: beam_width=7, max_depth=5, uniform policy.

### Fellowship (15 queries, ~160 files)

| System | Best NDCG@10 | Queries hit | P50 Latency |
|---|---|---|---|
| **SRT Rust daemon** | **0.628** | **15/15** | **6.6ms** |
| SRT Python | 0.619 | 14/15 | 5.1ms |
| Shire (FTS+RAG) | 0.410 | 10/15 | 3.9ms |
| ripgrep | 0.165 | 8/15 | 61.8ms |
| grep | 0.059 | 2/15 | 206.8ms |

See `bench/results/` for full reports (markdown + LaTeX with pgfplots).

### Running benchmarks

```bash
# Multi-system benchmark with parameter sweep
python3 bench/run_benchmark.py /path/to/repo bench/queries/repo.yaml \
  --systems srt-warm,grep,ripgrep,shire

# Analyze sweep results
python3 bench/analyze_sweep.py bench/results/repo-sweep.tsv
```

## Integration with AI agents

### Claude Code skill

The `srt-navigate` skill teaches Claude Code to use the SRT for code exploration:

1. Read `.sem/__dir__.md` at the relevant directory
2. For 15+ children, run `semtree query` to pre-filter
3. Descend into selected children
4. Read raw source files to answer

### Building the SRT with agents

The `srt-build` skill parallelizes SRT construction across multiple subagents, faster than the sequential CLI for initial builds.

## Design

Based on the paper: *Semantic Resolution Trees: Multi-Scale Context Management for AI Coding Agents* (see `docs/`).

Key design properties (all follow from storing summaries as plain text in git):

- **Zero infrastructure** — no vector DB, no server, just files
- **Git-native** — summaries version-controlled alongside code
- **Code-reviewable** — summary changes visible in diffs and PRs
- **Tool-agnostic** — any LLM generates, any agent consumes
- **Incremental** — only changed files re-summarized (hash-based)
- **Graceful fallback** — no `.sem/`? Agent searches normally

## License

MIT

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
| `semtree serve` | Start daemon for warm routing |
| `semtree bench <phase>` | Run benchmark evaluation |
| `semtree vec inspect <file>` | Inspect binary .vec file |

### Build options

```bash
semtree build /path/to/repo \
  --model claude-sonnet-4-20250514 \  # LLM for summarization
  --max-tokens 100000 \               # oversized file threshold
  --force \                            # rebuild all (ignore cache)
  --exclude 'vendor/*' \              # glob patterns to skip
  --no-embed \                        # skip embedding step
  --embed-model BAAI/bge-small-en-v1.5  # embedding model
```

### Route options

```bash
semtree route "your question" /path/to/repo \
  --beam-width 3 \   # children to explore per level (default: 3)
  --max-depth 10 \   # max descent depth (default: 10)
  --model BAAI/bge-small-en-v1.5
```

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
├── summarizer.rs    # LLM summarization via claude CLI
├── builder.rs       # Build pipeline orchestration
├── embedder.rs      # Embedding inference + cosine ranking
├── vec_store.rs     # Binary .vec format with mmap
├── server.rs        # Unix socket daemon (tokio)
└── bench.rs         # Benchmark data collection
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

Tested against 15 labeled queries on the [Fellowship](https://github.com/justinjdev/fellowship) repository (~160 files).

| System | Best NDCG@10 | Queries hit | P50 Latency |
|---|---|---|---|
| **SRT Rust daemon** | **0.628** | **15/15** | **6.6ms** |
| SRT Rust (cold) | 0.628 | 15/15 | 83.8ms |
| SRT Python | 0.619 | 14/15 | 5.1ms |
| Shire (FTS+RAG) | 0.410 | 10/15 | 3.9ms |
| ripgrep | 0.165 | 8/15 | 61.8ms |
| grep | 0.059 | 2/15 | 206.8ms |

**Hypervolume** (dominated region in latency-quality space):

| System | HV | 95% CI |
|---|---|---|
| SRT Rust daemon | 0.599 | [0.457, 0.736] |
| SRT Python | 0.566 | [0.430, 0.689] |
| Shire | 0.399 | [0.212, 0.595] |

SRT dominates on module-level and cross-cutting queries. Shire wins on focused symbol-name queries. See `bench/results/` for full reports.

### Running benchmarks

```bash
# Needs a repo with .sem/ records and .vec embeddings
PYTHONPATH=. semtree bench quality --repo-path /path/to/repo
PYTHONPATH=. semtree bench routing --repo-path /path/to/repo
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

Based on the paper: *Semantic Resolution Trees: Multi-Scale Context Management for AI Coding Agents* (see `docs/srt_v7.tex`).

Key design properties (all follow from storing summaries as plain text in git):

- **Zero infrastructure** — no vector DB, no server, just files
- **Git-native** — summaries version-controlled alongside code
- **Code-reviewable** — summary changes visible in diffs and PRs
- **Tool-agnostic** — any LLM generates, any agent consumes
- **Incremental** — only changed files re-summarized (hash-based)
- **Graceful fallback** — no `.sem/`? Agent searches normally

## License

MIT

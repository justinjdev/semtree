# semtree review — Review Manifest Generator

## Summary

A new `semtree review` CLI command that generates a structured review manifest for code changes. Given a git diff, it produces a markdown document with severity triage, semantic context per file, related files to read, and cross-cutting warnings for potentially missed changes. No LLM calls at review time — pure embedding math + record reading.

## Command Interface

```
semtree review [base..head] [path]
  --model BAAI/bge-small-en-v1.5    # embedding model
  --top-k 5                          # related files per changed file
  --similarity-threshold 0.7         # for cross-cutting warnings
```

- No args: defaults to uncommitted changes (unstaged, then staged — same pattern as `impact`)
- Explicit range: `main..HEAD`, `HEAD~3..HEAD`, `abc123..def456`
- Path: repository root (default `.`)

## Output Structure

### Section 1: Triage Table

Severity bucketed from embedding fan-out (count of files above similarity threshold to the changed file):
- **HIGH**: 10+ related files
- **MEDIUM**: 5-9 related files
- **LOW**: <5 related files

Format:

```markdown
## Triage

| File | Severity | Fan-out | Summary |
|------|----------|---------|---------|
| crates/engine/src/builder.rs | HIGH | 14 | Core execution orchestrator... |
| crates/unescape/src/lib.rs | LOW | 2 | String utility... |
```

Sorted by severity descending, then fan-out descending.

### Section 2: Per-File Context

For each changed file (ordered by severity):

```markdown
## crates/engine/src/builder.rs [HIGH]

**Summary:** Core execution orchestrator that builds and executes task dependency graphs...

**Module context (crates/engine/):** The turborepo-engine crate serves as the core
execution orchestrator for Turborepo...

**Related files to review:**
- crates/engine/src/lib.rs (0.89) — Engine public API and type exports
- crates/task-executor/src/visitor.rs (0.82) — Task graph visitor pattern
- crates/run-summary/src/execution.rs (0.78) — Execution tracking
- crates/engine/src/graph.rs (0.75) — Graph construction utilities
- crates/lib/src/run/mod.rs (0.71) — Run command entry point
```

Each related file includes its cosine similarity score and first-line summary from its `.sem/` record.

### Section 3: Cross-Cutting Warnings

Two tiers:

```markdown
## Cross-Cutting Warnings

### High Confidence
These files are explicitly documented as collaborators with changed files:
- **permissions.rs** not in diff — auth.rs and permissions.rs collaborate on session
  validation (from crates/auth/.sem/__dir__.md)

### Consider Also
These files are highly similar to changed files but not in the diff:
- crates/run-cache/src/lib.rs (0.74 similar to crates/task-executor/src/exec.rs)
- crates/ui/src/tui/pane.rs (0.72 similar to crates/process/src/child.rs)
```

## Implementation

### New module: `cli/src/review.rs`

Steps:

1. **Parse diff** — get changed file paths from `git diff --name-only [base..head]`. Reuse the pattern from `Commands::Impact` in main.rs (try unstaged, then staged, then error).

2. **Load semantic context** — for each changed file:
   - Read its `.sem/<file>.md` record (file summary)
   - Read its parent `<dir>/.sem/__dir__.md` record (module context)
   - Extract first line of summary for compact display

3. **Compute fan-out** — for each changed file:
   - Load its `.vec` embedding
   - Compute cosine similarity against all other `.vec` files in the repo
   - Count files above `--similarity-threshold` = fan-out score
   - Collect top-k as related files

4. **Bucket severity** — HIGH (10+), MEDIUM (5-9), LOW (<5) based on fan-out.

5. **Parse cross-cutting sections** — for each `__dir__.md` that is a parent of a changed file:
   - Extract the `## Cross-Cutting Concerns` section
   - Match file names mentioned against the changed file set
   - If a collaborator is mentioned but not in the diff, emit high-confidence warning

6. **Compute embedding warnings** — files above similarity threshold to any changed file, but not in the diff themselves. Deduplicate. These become "consider also" suggestions.

7. **Render markdown** — print to stdout in the three-section format above.

### Integration with main.rs

Add `Review` variant to `Commands` enum:

```rust
Review {
    /// Commit range (e.g., main..HEAD). Defaults to uncommitted changes.
    #[arg(default_value = "")]
    range: String,
    /// Repository root path
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Embedding model name
    #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
    model: String,
    /// Related files per changed file
    #[arg(long, default_value_t = 5)]
    top_k: usize,
    /// Cosine similarity threshold for cross-cutting warnings
    #[arg(long, default_value_t = 0.7)]
    similarity_threshold: f32,
}
```

### Reuse from existing code

- `embedder::embed_query` — embed text
- `embedder::cosine_similarity` — pairwise similarity
- `embedder::impact_analysis` — related file discovery (may refactor to share core logic)
- `records::read_record` — load `.sem/` records
- `vec_store::read_vec` — load `.vec` embeddings
- Git diff parsing pattern from `Commands::Impact`

### Performance

All operations are local: git diff, file reads, cosine similarity. No LLM calls, no network. For a repo with 6,000 `.vec` files and 20 changed files, this is ~120,000 cosine similarity computations — sub-second on any modern machine.

## Consumer

Output is markdown readable by both humans and AI agents. A review agent (Claude Code, Copilot, etc.) can ingest it as context before reviewing the actual diff.

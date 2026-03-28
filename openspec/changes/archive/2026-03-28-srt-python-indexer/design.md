## Context

This is a greenfield Python implementation of the Semantic Resolution Tree indexer as described in `docs/srt_v7.tex`. The repository currently contains only the paper and project configuration. The indexer is the paper's reference implementation — it needs to be correct, simple, and faithful to the spec.

The tool will be run against arbitrary git repositories to produce `.sem/` summary directories. It must work as a standalone CLI that developers install and run, with output that gets committed to git.

## Goals / Non-Goals

**Goals:**
- Faithful implementation of the paper's reference indexer (Appendix A)
- Deterministic, reproducible builds — same input produces same tree structure
- Incremental rebuilds via content hash comparison
- Crash-resumable — partial runs can be re-executed safely
- Clean CLI interface (`semtree build <path>`)
- Model-agnostic LLM integration (Anthropic as default, extensible to others)

**Non-Goals:**
- Query-time traversal protocol (that's the consuming agent's job, not the indexer's)
- Embedding-assisted routing (Appendix B optimization — future work)
- Symbol-level leaves (paper acknowledges this as a future extension)
- Batched summarization for high-fan-out directories (acknowledged limitation in the paper)
- Web UI, server mode, or any runtime infrastructure
- Supporting non-git repositories

## Decisions

### 1. Package structure: single `semtree` package with flat modules

The indexer has a small surface area. A flat module layout under `src/semtree/` avoids premature hierarchy:

```
src/semtree/
  __init__.py
  cli.py          # CLI entry point (argparse)
  walker.py       # Filesystem traversal, ignore rules
  hasher.py       # Content hashing (files and directories)
  summarizer.py   # LLM summarization (file + directory prompts)
  records.py      # .sem/ record reading/writing (YAML frontmatter + Markdown)
  config.py       # Configuration (model, context window size, API keys)
```

**Why over a deeper layout:** There are ~6 modules. Nesting adds indirection for no benefit at this scale.

### 2. Traversal: `os.walk` with post-order via collect-then-reverse

Python's `os.walk` with `topdown=False` gives bottom-up directory order. Files within each directory are sorted lexicographically for determinism. Ignore rules are applied during traversal:
- Skip entries starting with `.` (dotfiles, dot-directories)
- Skip symlinks (`os.path.islink`)
- Skip binary files (check for null bytes in first 8KB)

**Why not `pathlib.rglob`:** `os.walk` with `topdown=False` gives us bottom-up ordering natively, which is exactly what post-order DFS requires. `rglob` would require sorting into post-order manually.

### 3. Hashing: SHA-256, matching the paper's spec

- **Files:** SHA-256 of raw file contents (read as bytes)
- **Directories:** SHA-256 of the canonical string formed by sorting `(child_repo_relative_path, child_hash)` pairs lexicographically and joining them

This matches the paper's "git-style" hashing. The hash is stored in YAML frontmatter as `content_hash` and compared on subsequent runs to determine staleness.

**Why SHA-256 over actual git blob hashing:** The paper says "git-style" meaning content-addressed, not literally `git hash-object`. SHA-256 is simpler and avoids the git blob header format. The hash just needs to be deterministic and change when content changes.

### 4. LLM integration: Anthropic SDK as default, provider abstraction via a simple interface

A `Summarizer` protocol (Python `Protocol` class) with a single method: `summarize(prompt: str, content: str) -> str`. The default implementation uses the Anthropic SDK. Swapping providers means implementing the protocol.

**Why not LiteLLM or similar:** Adds a dependency for a problem we don't have yet. The paper is model-agnostic in principle, but the reference implementation should be concrete. Start with one provider, abstract later if needed.

### 5. Record format: YAML frontmatter via manual string formatting, not a library

Records are simple enough to write with f-strings:

```yaml
---
path: src/foo/bar.py
type: file
content_hash: a1b2c3d4...
---

Summary text here.
```

For reading, split on `---` delimiters and parse the YAML block with PyYAML.

**Why not python-frontmatter library:** One less dependency. The format is three fields of YAML followed by Markdown. String split + `yaml.safe_load` handles it.

### 6. Oversized file handling: configurable token limit, not character limit

The paper says "fits context window." We'll estimate tokens as `len(content) / 4` (rough bytes-to-tokens approximation) and compare against a configurable max (default: 100K tokens). Oversized files get the placeholder `summary unavailable: oversized file`.

**Why not tiktoken:** Adds a heavy dependency for a heuristic check. The approximation is sufficient — the exact boundary doesn't matter much since the LLM will handle slightly-over files fine, and truly oversized files are orders of magnitude over the limit.

### 7. CLI: argparse, single `build` command

```
semtree build [path] [--model MODEL] [--max-tokens N] [--force]
```

- `path`: target repository root (defaults to cwd)
- `--model`: LLM model to use (default: `claude-sonnet-4-20250514`)
- `--max-tokens`: max file tokens before marking oversized (default: 100000)
- `--force`: rebuild all nodes regardless of hash match

**Why argparse over click/typer:** Standard library, no dependency. The CLI is one command with a few flags.

### 8. Concurrency: sequential file summarization, no parallelism in v1

The paper's algorithm shows file summarization as parallelizable. However, for the reference implementation:
- Sequential is simpler to debug and reason about
- LLM API rate limits are the real bottleneck, not local processing
- `asyncio` with a semaphore is the natural future optimization

**Why not parallel from the start:** Correct sequential behavior first, optimize later. The paper itself says a full rebuild is viable when cost permits.

## Risks / Trade-offs

- **LLM API cost on large repos** → Incremental rebuild mitigates this for subsequent runs. First build of a large repo will be expensive. Document expected costs in README.
- **Binary detection heuristic (null byte check) has false positives/negatives** → Acceptable for the reference implementation. Files with null bytes in the first 8KB are almost always binary. Edge cases (e.g., UTF-16 files) can be handled later.
- **Token estimation is approximate** → A file slightly over the limit might be skipped unnecessarily, or slightly under might cause an API error. The 4:1 ratio is conservative enough. Can be refined later.
- **No `.gitignore` integration in v1** → The ignore rules (dotfiles, symlinks, binaries) cover the most common cases. Adding `.gitignore` parsing (via `pathspec` library) is a natural follow-up but adds a dependency.
- **High-fan-out directories may exceed context window** → The paper acknowledges this limitation. For v1, the build fails for that node. Document this as a known limitation.

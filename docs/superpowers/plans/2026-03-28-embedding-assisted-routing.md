# Embedding-Assisted Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add embedding-assisted pre-filtering to SRT so high fan-out directories can be cheaply narrowed by cosine similarity before LLM routing.

**Architecture:** A new `embedder.py` module handles fastembed initialization, `.vec` file I/O, and cosine ranking. The builder pipeline calls it after summarization. Two new CLI subcommands (`embed`, `query`) expose standalone embedding and query-time ranking. The navigate skill gets updated instructions for when to use `semtree query`.

**Tech Stack:** Python 3.11+, fastembed (ONNX-based local embeddings), numpy (transitive via fastembed), pyyaml (existing)

**Spec:** `docs/superpowers/specs/2026-03-28-embedding-assisted-routing-design.md`

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/semtree/embedder.py` | Create | fastembed wrapper, `.vec` I/O, cosine ranking |
| `src/semtree/builder.py` | Modify | Call embedder after each node's summary is written |
| `src/semtree/cli.py` | Modify | Add `embed` and `query` subcommands, `--no-embed` flag on `build` |
| `src/semtree/config.py` | Modify | Add embed-related fields to `BuildConfig` |
| `.claude/skills/srt-navigate/SKILL.md` | Modify | Add pre-filter step for high fan-out directories |
| `pyproject.toml` | Modify | Add `fastembed` dependency |
| `tests/test_embedder.py` | Create | Unit tests for embedder module |
| `tests/test_embed_cli.py` | Create | Integration tests for `embed` and `query` CLI commands |

---

### Task 1: Add fastembed dependency

**Files:**
- Modify: `pyproject.toml`

- [ ] **Step 1: Add fastembed to dependencies**

In `pyproject.toml`, add `fastembed` to the dependencies list:

```toml
dependencies = [
    "pyyaml>=6.0",
    "fastembed>=0.6",
]
```

- [ ] **Step 2: Install and verify**

Run: `cd /Users/justin/git/semtree && pip install -e '.[dev]' 2>&1 | tail -5`

Verify fastembed imports:

Run: `python -c "from fastembed import TextEmbedding; print('ok')"`
Expected: `ok`

- [ ] **Step 3: Commit**

```bash
git add pyproject.toml
git commit -m "feat: add fastembed dependency for embedding-assisted routing"
```

---

### Task 2: Embedder module — `.vec` I/O and freshness check

**Files:**
- Create: `src/semtree/embedder.py`
- Create: `tests/test_embedder.py`

- [ ] **Step 1: Write failing tests for `.vec` I/O**

Create `tests/test_embedder.py`:

```python
"""Tests for semtree.embedder module."""

import json
from pathlib import Path

import pytest

from semtree.embedder import read_vec, write_vec, is_vec_fresh


class TestWriteVec:
    def test_creates_file_with_expected_fields(self, tmp_path: Path) -> None:
        vec_path = tmp_path / ".sem" / "foo.py.vec"
        write_vec(vec_path, model="BAAI/bge-small-en-v1.5", content_hash="abc123", vector=[0.1, -0.2, 0.3])

        assert vec_path.exists()
        data = json.loads(vec_path.read_text())
        assert data["model"] == "BAAI/bge-small-en-v1.5"
        assert data["content_hash"] == "abc123"
        assert data["vector"] == [0.1, -0.2, 0.3]

    def test_creates_parent_directories(self, tmp_path: Path) -> None:
        vec_path = tmp_path / "deep" / "nested" / ".sem" / "bar.py.vec"
        write_vec(vec_path, model="m", content_hash="h", vector=[1.0])
        assert vec_path.exists()


class TestReadVec:
    def test_returns_none_for_missing_file(self, tmp_path: Path) -> None:
        assert read_vec(tmp_path / "missing.vec") is None

    def test_reads_written_vec(self, tmp_path: Path) -> None:
        vec_path = tmp_path / ".sem" / "foo.py.vec"
        write_vec(vec_path, model="m", content_hash="h", vector=[1.0, 2.0])

        data = read_vec(vec_path)
        assert data is not None
        assert data["model"] == "m"
        assert data["content_hash"] == "h"
        assert data["vector"] == [1.0, 2.0]

    def test_returns_none_for_invalid_json(self, tmp_path: Path) -> None:
        bad = tmp_path / "bad.vec"
        bad.parent.mkdir(parents=True, exist_ok=True)
        bad.write_text("not json")
        assert read_vec(bad) is None


class TestIsVecFresh:
    def test_fresh_when_hash_and_model_match(self) -> None:
        existing = {"model": "m", "content_hash": "h", "vector": [1.0]}
        assert is_vec_fresh(existing, content_hash="h", model="m") is True

    def test_stale_when_hash_differs(self) -> None:
        existing = {"model": "m", "content_hash": "old", "vector": [1.0]}
        assert is_vec_fresh(existing, content_hash="new", model="m") is False

    def test_stale_when_model_differs(self) -> None:
        existing = {"model": "old-model", "content_hash": "h", "vector": [1.0]}
        assert is_vec_fresh(existing, content_hash="h", model="new-model") is False

    def test_stale_when_none(self) -> None:
        assert is_vec_fresh(None, content_hash="h", model="m") is False
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embedder.py -v 2>&1 | tail -20`
Expected: ImportError — `cannot import name 'read_vec' from 'semtree.embedder'`

- [ ] **Step 3: Implement `.vec` I/O functions**

Create `src/semtree/embedder.py`:

```python
"""Embedding-assisted routing: fastembed wrapper, .vec I/O, cosine ranking."""

import json
from pathlib import Path
from typing import Any


DEFAULT_MODEL = "BAAI/bge-small-en-v1.5"


def write_vec(
    vec_path: Path,
    model: str,
    content_hash: str,
    vector: list[float],
) -> None:
    """Write a .vec sidecar file with embedding data."""
    vec_path.parent.mkdir(parents=True, exist_ok=True)
    data = {"model": model, "content_hash": content_hash, "vector": vector}
    vec_path.write_text(json.dumps(data), encoding="utf-8")


def read_vec(vec_path: Path) -> dict[str, Any] | None:
    """Read a .vec sidecar file, or None if missing/invalid."""
    if not vec_path.exists():
        return None
    try:
        return json.loads(vec_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def is_vec_fresh(
    existing: dict[str, Any] | None,
    content_hash: str,
    model: str,
) -> bool:
    """Check if a .vec file is up-to-date."""
    if existing is None:
        return False
    return existing.get("content_hash") == content_hash and existing.get("model") == model
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embedder.py -v 2>&1 | tail -20`
Expected: All 8 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/semtree/embedder.py tests/test_embedder.py
git commit -m "feat: add .vec I/O and freshness check for embeddings"
```

---

### Task 3: Embedder module — fastembed wrapper and cosine ranking

**Files:**
- Modify: `src/semtree/embedder.py`
- Modify: `tests/test_embedder.py`

- [ ] **Step 1: Write failing tests for embedding and ranking**

Append to `tests/test_embedder.py`:

```python
import numpy as np

from semtree.embedder import embed_texts, embed_query, cosine_rank


class TestEmbedTexts:
    def test_returns_list_of_vectors(self) -> None:
        vectors = embed_texts(["hello world", "foo bar"])
        assert len(vectors) == 2
        assert len(vectors[0]) > 0
        # Vectors should be lists of floats
        assert isinstance(vectors[0][0], float)

    def test_empty_input_returns_empty(self) -> None:
        assert embed_texts([]) == []


class TestEmbedQuery:
    def test_returns_single_vector(self) -> None:
        vector = embed_query("how does auth work?")
        assert len(vector) > 0
        assert isinstance(vector[0], float)


class TestCosineRank:
    def test_ranks_by_similarity(self) -> None:
        # query_vec is close to child_b, far from child_a
        query_vec = [1.0, 0.0, 0.0]
        children = {
            "a": [0.0, 1.0, 0.0],  # orthogonal
            "b": [0.9, 0.1, 0.0],  # close
            "c": [0.5, 0.5, 0.0],  # medium
        }
        ranked = cosine_rank(query_vec, children)
        paths = [path for path, _score in ranked]
        assert paths[0] == "b"
        assert paths[-1] == "a"

    def test_empty_children_returns_empty(self) -> None:
        assert cosine_rank([1.0, 0.0], {}) == []
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embedder.py::TestEmbedTexts -v 2>&1 | tail -10`
Expected: ImportError — `cannot import name 'embed_texts'`

- [ ] **Step 3: Implement fastembed wrapper and cosine ranking**

Add to `src/semtree/embedder.py`, below the existing functions:

```python
import numpy as np
from fastembed import TextEmbedding

# Module-level lazy singleton to avoid re-loading the model on every call.
_model_cache: dict[str, TextEmbedding] = {}


def _get_model(model_name: str = DEFAULT_MODEL) -> TextEmbedding:
    """Return a cached TextEmbedding instance."""
    if model_name not in _model_cache:
        _model_cache[model_name] = TextEmbedding(model_name=model_name)
    return _model_cache[model_name]


def embed_texts(
    texts: list[str],
    model_name: str = DEFAULT_MODEL,
) -> list[list[float]]:
    """Embed a batch of document texts. Returns list of float vectors."""
    if not texts:
        return []
    model = _get_model(model_name)
    embeddings = list(model.passage_embed(texts))
    return [vec.tolist() for vec in embeddings]


def embed_query(
    query: str,
    model_name: str = DEFAULT_MODEL,
) -> list[float]:
    """Embed a single query string. Returns a float vector."""
    model = _get_model(model_name)
    embeddings = list(model.query_embed(query))
    return embeddings[0].tolist()


def cosine_rank(
    query_vec: list[float],
    children: dict[str, list[float]],
) -> list[tuple[str, float]]:
    """Rank children by cosine similarity to query. Returns [(path, score)] descending."""
    if not children:
        return []
    q = np.array(query_vec)
    q_norm = np.linalg.norm(q)
    if q_norm == 0:
        return [(path, 0.0) for path in children]

    results = []
    for path, vec in children.items():
        c = np.array(vec)
        c_norm = np.linalg.norm(c)
        if c_norm == 0:
            results.append((path, 0.0))
        else:
            score = float(np.dot(q, c) / (q_norm * c_norm))
            results.append((path, score))

    results.sort(key=lambda x: x[1], reverse=True)
    return results
```

Note: Move the `import numpy as np` to the top of the file alongside the other imports. Add `from fastembed import TextEmbedding` at the top as well.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embedder.py -v 2>&1 | tail -20`
Expected: All 13 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/semtree/embedder.py tests/test_embedder.py
git commit -m "feat: add fastembed wrapper and cosine ranking"
```

---

### Task 4: `semtree embed` CLI command

**Files:**
- Modify: `src/semtree/cli.py`
- Modify: `src/semtree/embedder.py`
- Create: `tests/test_embed_cli.py`

- [ ] **Step 1: Write failing test for embed command**

Create `tests/test_embed_cli.py`:

```python
"""Integration tests for semtree embed and query CLI commands."""

import json
from pathlib import Path

import pytest

from semtree.embedder import write_vec, read_vec, DEFAULT_MODEL
from semtree.records import write_record


def _make_sem_tree(tmp_path: Path) -> None:
    """Create a minimal .sem/ tree with two file records and a dir record."""
    sem = tmp_path / ".sem"
    sem.mkdir()

    write_record(sem / "foo.py.md", "foo.py", "file", "hash_foo", "Handles user login.")
    write_record(sem / "bar.py.md", "bar.py", "file", "hash_bar", "Renders HTML templates.")
    write_record(sem / "__dir__.md", ".", "directory", "hash_dir", "Root module.\n\n## Children\n\n- **foo.py**: Handles user login.\n- **bar.py**: Renders HTML templates.")


class TestEmbedCommand:
    def test_creates_vec_files(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory

        stats = embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)

        assert (tmp_path / ".sem" / "foo.py.vec").exists()
        assert (tmp_path / ".sem" / "bar.py.vec").exists()
        assert (tmp_path / ".sem" / "__dir__.vec").exists()
        assert stats["embedded"] == 3
        assert stats["skipped"] == 0

    def test_skips_fresh_vec_files(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory

        # First run embeds everything
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)
        # Second run should skip all
        stats = embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)

        assert stats["embedded"] == 0
        assert stats["skipped"] == 3

    def test_force_reembeds(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory

        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)
        stats = embed_directory(tmp_path, model=DEFAULT_MODEL, force=True)

        assert stats["embedded"] == 3
        assert stats["skipped"] == 0
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embed_cli.py::TestEmbedCommand::test_creates_vec_files -v 2>&1 | tail -10`
Expected: ImportError — `cannot import name 'embed_directory'`

- [ ] **Step 3: Implement `embed_directory` function**

Add to `src/semtree/embedder.py`:

```python
from semtree.records import read_record, SEM_DIR


def _find_sem_records(target_path: Path) -> list[tuple[Path, Path]]:
    """Find all (.md record, .vec sidecar) pairs under target_path.

    Returns list of (md_path, vec_path) tuples.
    """
    pairs = []
    for md_path in sorted(target_path.rglob(f"{SEM_DIR}/*.md")):
        vec_path = md_path.with_suffix(".vec")
        pairs.append((md_path, vec_path))
    return pairs


def embed_directory(
    target_path: Path,
    model: str = DEFAULT_MODEL,
    force: bool = False,
) -> dict[str, int]:
    """Embed all .sem/ records under target_path. Returns stats dict."""
    pairs = _find_sem_records(target_path)
    stats = {"embedded": 0, "skipped": 0, "errored": 0}

    # Collect texts that need embedding
    to_embed: list[tuple[Path, str]] = []  # (vec_path, summary)
    for md_path, vec_path in pairs:
        record = read_record(md_path)
        if record is None:
            stats["errored"] += 1
            continue

        content_hash = record.get("content_hash", "")
        summary = record.get("summary", "")

        if not force:
            existing = read_vec(vec_path)
            if is_vec_fresh(existing, content_hash, model):
                stats["skipped"] += 1
                continue

        to_embed.append((vec_path, content_hash, summary))

    if not to_embed:
        return stats

    # Batch embed all summaries at once
    texts = [summary for _, _, summary in to_embed]
    vectors = embed_texts(texts, model_name=model)

    for (vec_path, content_hash, _), vector in zip(to_embed, vectors):
        write_vec(vec_path, model=model, content_hash=content_hash, vector=vector)
        stats["embedded"] += 1

    return stats
```

Note: The `to_embed` list items are `(vec_path, content_hash, summary)` tuples — update the type hint comment to match.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embed_cli.py::TestEmbedCommand -v 2>&1 | tail -15`
Expected: All 3 tests PASS

- [ ] **Step 5: Wire up the CLI subcommand**

In `src/semtree/cli.py`, add the `embed` subcommand. After the `build_parser` block (around line 47), add:

```python
    embed_parser = sub.add_parser("embed", help="Compute embeddings for existing .sem/ records")
    embed_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Repository root path (default: current directory)",
    )
    embed_parser.add_argument(
        "--model",
        default="BAAI/bge-small-en-v1.5",
        help="Embedding model name (default: BAAI/bge-small-en-v1.5)",
    )
    embed_parser.add_argument(
        "--force",
        action="store_true",
        help="Re-embed all records, ignoring freshness checks",
    )
```

At the bottom of `main()`, add the handler (after the `if args.command == "build":` block):

```python
    elif args.command == "embed":
        target = Path(args.path).resolve()
        if not target.is_dir():
            print(f"error: {args.path} is not a directory", file=sys.stderr)
            sys.exit(1)

        from semtree.embedder import embed_directory

        stats = embed_directory(target, model=args.model, force=args.force)
        print(
            f"Done: {stats['embedded']} embedded, "
            f"{stats['skipped']} skipped, "
            f"{stats['errored']} errored",
            file=sys.stderr,
        )
```

- [ ] **Step 6: Manual smoke test**

Run: `cd /Users/justin/git/semtree && semtree embed .`
Expected: Embeds all existing `.sem/` records, creates `.vec` files alongside them.

- [ ] **Step 7: Commit**

```bash
git add src/semtree/embedder.py src/semtree/cli.py tests/test_embed_cli.py
git commit -m "feat: add semtree embed command for standalone embedding"
```

---

### Task 5: `semtree query` CLI command

**Files:**
- Modify: `src/semtree/cli.py`
- Modify: `src/semtree/embedder.py`
- Modify: `tests/test_embed_cli.py`

- [ ] **Step 1: Write failing tests for query**

Append to `tests/test_embed_cli.py`:

```python
from semtree.embedder import query_directory


class TestQueryCommand:
    def test_returns_ranked_children(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory

        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)

        results = query_directory(tmp_path, query="user authentication login", model=DEFAULT_MODEL)

        # Should return list of (score, path, summary_first_line) tuples
        assert len(results) > 0
        # Each result is (score, path, first_line)
        score, path, first_line = results[0]
        assert isinstance(score, float)
        assert isinstance(path, str)
        assert isinstance(first_line, str)

    def test_returns_empty_when_no_vec_files(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        results = query_directory(tmp_path, query="anything", model=DEFAULT_MODEL)
        assert results == []

    def test_top_k_limits_results(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory

        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)

        results = query_directory(tmp_path, query="login", model=DEFAULT_MODEL, top_k=1)
        assert len(results) == 1

    def test_threshold_filters_results(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory

        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)

        # Very high threshold should filter most/all results
        results = query_directory(tmp_path, query="login", model=DEFAULT_MODEL, threshold=0.99)
        assert len(results) < 3  # At least some filtered
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embed_cli.py::TestQueryCommand::test_returns_ranked_children -v 2>&1 | tail -10`
Expected: ImportError — `cannot import name 'query_directory'`

- [ ] **Step 3: Implement `query_directory` function**

Add to `src/semtree/embedder.py`:

```python
def query_directory(
    target_path: Path,
    query: str,
    model: str = DEFAULT_MODEL,
    top_k: int | None = None,
    threshold: float | None = None,
) -> list[tuple[float, str, str]]:
    """Query children of a directory by cosine similarity.

    Returns [(score, repo_relative_path, summary_first_line)] ranked descending.
    Only considers immediate children of target_path (files in target_path/.sem/).
    """
    sem_dir = target_path / SEM_DIR
    if not sem_dir.is_dir():
        return []

    # Load child vectors and summaries
    children_vecs: dict[str, list[float]] = {}
    children_summaries: dict[str, str] = {}

    for vec_path in sorted(sem_dir.glob("*.vec")):
        vec_data = read_vec(vec_path)
        if vec_data is None:
            continue

        # Find corresponding .md record for the summary
        md_path = vec_path.with_suffix(".md")
        record = read_record(md_path)
        if record is None:
            continue

        rel_path = record.get("path", "")
        summary = record.get("summary", "")
        first_line = summary.split("\n", 1)[0].strip()

        children_vecs[rel_path] = vec_data["vector"]
        children_summaries[rel_path] = first_line

    if not children_vecs:
        return []

    query_vec = embed_query(query, model_name=model)
    ranked = cosine_rank(query_vec, children_vecs)

    results = []
    for path, score in ranked:
        if threshold is not None and score < threshold:
            continue
        first_line = children_summaries.get(path, "")
        results.append((score, path, first_line))

    if top_k is not None:
        results = results[:top_k]

    return results
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/test_embed_cli.py::TestQueryCommand -v 2>&1 | tail -15`
Expected: All 4 tests PASS

- [ ] **Step 5: Wire up the CLI subcommand**

In `src/semtree/cli.py`, add the `query` subcommand. After the `embed_parser` block, add:

```python
    query_parser = sub.add_parser("query", help="Rank directory children by similarity to a query")
    query_parser.add_argument(
        "query",
        help="Natural language query",
    )
    query_parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Directory whose children to rank (default: current directory)",
    )
    query_parser.add_argument(
        "--model",
        default="BAAI/bge-small-en-v1.5",
        help="Embedding model name (default: BAAI/bge-small-en-v1.5)",
    )
    query_parser.add_argument(
        "--top-k",
        type=int,
        default=None,
        help="Return only top K results",
    )
    query_parser.add_argument(
        "--threshold",
        type=float,
        default=None,
        help="Minimum cosine similarity score",
    )
```

Add the handler:

```python
    elif args.command == "query":
        target = Path(args.path).resolve()
        if not target.is_dir():
            print(f"error: {args.path} is not a directory", file=sys.stderr)
            sys.exit(1)

        from semtree.embedder import query_directory

        results = query_directory(
            target,
            query=args.query,
            model=args.model,
            top_k=args.top_k,
            threshold=args.threshold,
        )

        if not results:
            print("No results (missing .vec files? Run: semtree embed)", file=sys.stderr)
            sys.exit(1)

        for score, path, first_line in results:
            print(f"{score:.4f}\t{path}\t{first_line}")
```

- [ ] **Step 6: Manual smoke test**

Run: `cd /Users/justin/git/semtree && semtree query "how does the builder pipeline work?"`
Expected: Ranked list of children with scores, paths, and summary first lines.

- [ ] **Step 7: Commit**

```bash
git add src/semtree/embedder.py src/semtree/cli.py tests/test_embed_cli.py
git commit -m "feat: add semtree query command for cosine-ranked child retrieval"
```

---

### Task 6: Integrate embedding into `semtree build`

**Files:**
- Modify: `src/semtree/config.py`
- Modify: `src/semtree/builder.py`
- Modify: `src/semtree/cli.py`

- [ ] **Step 1: Add embed fields to BuildConfig**

In `src/semtree/config.py`, add two fields:

```python
@dataclass(frozen=True)
class BuildConfig:
    target_path: Path
    model: str = "claude-sonnet-4-20250514"
    max_tokens: int = 100_000
    force: bool = False
    exclude: tuple[str, ...] = ()
    embed: bool = True
    embed_model: str = "BAAI/bge-small-en-v1.5"
```

- [ ] **Step 2: Add embedding step to builder**

In `src/semtree/builder.py`, after all nodes are processed (after the `for` loop ends, before the final stats print), add:

```python
    if config.embed:
        from semtree.embedder import embed_directory

        print("\nComputing embeddings...", file=sys.stderr)
        embed_stats = embed_directory(
            config.target_path,
            model=config.embed_model,
            force=config.force,
        )
        print(
            f"Embeddings: {embed_stats['embedded']} embedded, "
            f"{embed_stats['skipped']} skipped, "
            f"{embed_stats['errored']} errored",
            file=sys.stderr,
        )
```

- [ ] **Step 3: Add `--no-embed` flag to CLI**

In `src/semtree/cli.py`, add to the `build_parser` arguments:

```python
    build_parser.add_argument(
        "--no-embed",
        action="store_true",
        help="Skip embedding computation after build",
    )
    build_parser.add_argument(
        "--embed-model",
        default="BAAI/bge-small-en-v1.5",
        help="Embedding model name (default: BAAI/bge-small-en-v1.5)",
    )
```

Update the `BuildConfig` construction in the `build` handler:

```python
        config = BuildConfig(
            target_path=target,
            model=args.model,
            max_tokens=args.max_tokens,
            force=args.force,
            exclude=tuple(args.exclude),
            embed=not args.no_embed,
            embed_model=args.embed_model,
        )
```

- [ ] **Step 4: Run all existing tests**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/ -v 2>&1 | tail -20`
Expected: All tests pass. The e2e build tests use a mock summarizer and won't trigger embedding (they don't produce real `.sem/` records in the right shape), but config changes should not break them.

- [ ] **Step 5: Commit**

```bash
git add src/semtree/config.py src/semtree/builder.py src/semtree/cli.py
git commit -m "feat: integrate embedding into semtree build with --no-embed flag"
```

---

### Task 7: Update `srt-navigate` skill

**Files:**
- Modify: `.claude/skills/srt-navigate/SKILL.md`

- [ ] **Step 1: Add pre-filter step to the protocol**

In `.claude/skills/srt-navigate/SKILL.md`, between "Step 1: Enter via the routing table" and "Step 2: Descend through summaries", insert:

```markdown
### Step 1.5: Pre-filter high fan-out directories

If the directory from Step 1 has **15 or more children** listed in its `## Children` section:

1. Run: `semtree query "<your question>" <directory-path>`
2. Use the top-ranked results to decide which children to descend into
3. This replaces manual scanning of all children — the cosine ranking does the initial triage

If `semtree` is not available or the directory has fewer than 15 children, skip this step and scan the children list manually as before.
```

- [ ] **Step 2: Verify skill file is well-formed**

Read the skill file and confirm the steps flow logically: Step 1 -> Step 1.5 (conditional) -> Step 2 -> Step 3 -> Step 4.

- [ ] **Step 3: Commit**

```bash
git add .claude/skills/srt-navigate/SKILL.md
git commit -m "feat: add embedding pre-filter step to srt-navigate skill"
```

---

### Task 8: Run full test suite and smoke test

- [ ] **Step 1: Run all tests**

Run: `cd /Users/justin/git/semtree && python -m pytest tests/ -v`
Expected: All tests pass.

- [ ] **Step 2: End-to-end smoke test on the semtree repo itself**

Run: `cd /Users/justin/git/semtree && semtree embed . 2>&1 | tail -5`
Expected: Creates `.vec` files next to all existing `.sem/*.md` records.

Run: `cd /Users/justin/git/semtree && semtree query "how does the build pipeline work?" .`
Expected: Ranked list showing `src/` and similar entries near the top.

Run: `cd /Users/justin/git/semtree && semtree query "how does the build pipeline work?" src/semtree`
Expected: Ranked list with `builder.py`, `summarizer.py`, `walker.py` scoring high.

- [ ] **Step 3: Verify `.vec` file content is reasonable**

Run: `cd /Users/justin/git/semtree && cat .sem/src.vec 2>/dev/null || cat $(find . -name "*.vec" -path "*/.sem/*" | head -1)`
Expected: Valid JSON with `model`, `content_hash`, and `vector` fields.

- [ ] **Step 4: Final commit if any fixups were needed**

```bash
git add -A && git commit -m "fix: address issues found during smoke testing"
```

Only run this if Step 1-3 revealed issues that required fixes.

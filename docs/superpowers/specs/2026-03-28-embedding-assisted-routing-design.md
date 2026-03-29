# Embedding-Assisted Routing for SRT

**Date:** 2026-03-28
**Status:** Approved
**Spec reference:** `docs/srt_v7.tex` Appendix B (Embedding-Assisted Routing)

## Summary

Add optional embedding-assisted pre-filtering to the SRT query-time protocol. At high fan-out directory nodes, pre-rank children by cosine similarity between a query embedding and precomputed node embeddings before the LLM makes a routing decision. Embeddings are computed offline using fastembed (local inference, no API calls) and stored as per-node `.vec` sidecar files in `.sem/` directories.

## Motivation

The current query-time protocol reads all child summaries and asks the LLM to select relevant branches. This works well for moderate branching (b <= 10) but becomes expensive at high fan-out nodes (30+ children) where concatenating all summaries is costly or exceeds prompt limits. Precomputed embeddings allow cheap cosine pre-filtering to narrow the candidate set before the LLM call.

## Design

### Storage format

Per-node `.vec` sidecar files colocated with `.md` records:
- `.sem/foo.py.vec` next to `.sem/foo.py.md`
- `.sem/__dir__.vec` next to `.sem/__dir__.md`

JSON format:
```json
{
  "model": "BAAI/bge-small-en-v1.5",
  "content_hash": "deadbeef...",
  "vector": [0.0123, -0.0456, ...]
}
```

- `content_hash` matches the summary record's hash. If they differ, the embedding is stale.
- `model` detects when the embedding model changes (requiring full re-embed).

### Embedding module (`src/semtree/embedder.py`)

New module providing:

1. `embed_texts(texts: list[str]) -> list[list[float]]` — batch document embedding via fastembed
2. `embed_query(query: str) -> list[float]` — single query embedding (fastembed distinguishes document vs query prefixes)
3. `read_vec(path) -> dict` — read `.vec` sidecar file
4. `write_vec(path, model, content_hash, vector)` — write `.vec` sidecar file
5. Freshness check: compare `content_hash` and `model` from `.vec` against current values, skip if both match

Uses fastembed with configurable model (default `BAAI/bge-small-en-v1.5`). fastembed runs locally via ONNX Runtime — no external API calls, consistent with SRT's zero-infrastructure design.

### CLI commands

**`semtree embed [path]`** — standalone command to embed existing summaries
- Walks `.sem/` directories under `path` (default: repo root)
- Reads each `.md` record's summary and `content_hash`
- Skips nodes where `.vec` exists with matching `content_hash` and `model`
- Writes `.vec` sidecars for the rest
- Flags: `--model <name>` (default `BAAI/bge-small-en-v1.5`), `--force` (re-embed all)

**`semtree build`** — updated to include embedding after summarization
- After writing each `.md` record, also computes and writes the `.vec` sidecar
- Flag: `--no-embed` to skip embedding

**`semtree query <query> [path]`** — query-time ranking
- `path` defaults to repo root, specifies which directory's children to rank
- Reads all child `.vec` files under `path/.sem/`
- Embeds the query with fastembed
- Returns children ranked by cosine similarity: `score path summary_first_line`
- Flags: `--top-k <n>` (default: all), `--threshold <float>` (minimum similarity, default: none)

### Navigate skill update

The `srt-navigate` skill (`.claude/skills/srt-navigate/SKILL.md`) gets a new step between "Enter via routing table" and "Descend through summaries":

> **Pre-filter (high fan-out):** If a directory has 15+ children, run `semtree query "<your question>" <dir>` to get children ranked by relevance. Use the top results to guide which branches to descend into. For directories with fewer children, read summaries directly as before.

### Incremental behavior

Same hash-based invalidation as summaries:
- If a node's `content_hash` hasn't changed and the model is the same, skip embedding
- If the embedding model changes, `--force` or a model mismatch triggers full re-embed
- Changes propagate naturally: a changed file gets a new summary, which gets a new hash, which triggers a new embedding

### Dependencies

- `fastembed` added as a required dependency in `pyproject.toml` (brings numpy and onnxruntime)

## File inventory

| File | Action |
|---|---|
| `src/semtree/embedder.py` | New |
| `src/semtree/builder.py` | Modify |
| `src/semtree/cli.py` | Modify |
| `.claude/skills/srt-navigate/SKILL.md` | Modify |
| `pyproject.toml` | Modify |
| `tests/test_embedder.py` | New |
| `tests/test_query.py` | New |

"""Embedding-assisted routing: fastembed wrapper, .vec I/O, cosine ranking."""

import json
from pathlib import Path
from typing import Any

import numpy as np
from fastembed import TextEmbedding

from semtree.records import read_record, SEM_DIR


DEFAULT_MODEL = "BAAI/bge-small-en-v1.5"

# Module-level lazy singleton to avoid re-loading the model on every call.
_model_cache: dict[str, TextEmbedding] = {}


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


def _find_sem_records(target_path: Path) -> list[tuple[Path, Path]]:
    """Find all (.md record, .vec sidecar) pairs under target_path."""
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

    to_embed: list[tuple[Path, str, str]] = []  # (vec_path, content_hash, summary)
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

    texts = [summary for _, _, summary in to_embed]
    vectors = embed_texts(texts, model_name=model)

    for (vec_path, content_hash, _), vector in zip(to_embed, vectors):
        write_vec(vec_path, model=model, content_hash=content_hash, vector=vector)
        stats["embedded"] += 1

    return stats


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

    children_vecs: dict[str, list[float]] = {}
    children_summaries: dict[str, str] = {}

    for vec_path in sorted(sem_dir.glob("*.vec")):
        vec_data = read_vec(vec_path)
        if vec_data is None:
            continue

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

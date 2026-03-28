"""Embedding-assisted routing: fastembed wrapper, .vec I/O, cosine ranking."""

import json
from pathlib import Path
from typing import Any

import numpy as np
from fastembed import TextEmbedding


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

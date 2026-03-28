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

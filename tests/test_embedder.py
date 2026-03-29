"""Tests for semtree.embedder module."""

import json
from pathlib import Path

import pytest

import numpy as np

from semtree.embedder import read_vec, write_vec, is_vec_fresh, embed_texts, embed_query, cosine_rank


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


class TestEmbedTexts:
    def test_returns_list_of_vectors(self) -> None:
        vectors = embed_texts(["hello world", "foo bar"])
        assert len(vectors) == 2
        assert len(vectors[0]) > 0
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
        query_vec = [1.0, 0.0, 0.0]
        children = {
            "a": [0.0, 1.0, 0.0],
            "b": [0.9, 0.1, 0.0],
            "c": [0.5, 0.5, 0.0],
        }
        ranked = cosine_rank(query_vec, children)
        paths = [path for path, _score in ranked]
        assert paths[0] == "b"
        assert paths[-1] == "a"

    def test_empty_children_returns_empty(self) -> None:
        assert cosine_rank([1.0, 0.0], {}) == []

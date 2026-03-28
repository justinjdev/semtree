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


class TestEmbedDirectory:
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
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)
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


class TestQueryDirectory:
    def test_returns_ranked_children(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory, query_directory
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)

        results = query_directory(tmp_path, query="user authentication login", model=DEFAULT_MODEL)
        assert len(results) > 0
        score, path, first_line = results[0]
        assert isinstance(score, float)
        assert isinstance(path, str)
        assert isinstance(first_line, str)

    def test_returns_empty_when_no_vec_files(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import query_directory
        results = query_directory(tmp_path, query="anything", model=DEFAULT_MODEL)
        assert results == []

    def test_top_k_limits_results(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory, query_directory
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)
        results = query_directory(tmp_path, query="login", model=DEFAULT_MODEL, top_k=1)
        assert len(results) == 1

    def test_threshold_filters_results(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory, query_directory
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=False)
        results = query_directory(tmp_path, query="login", model=DEFAULT_MODEL, threshold=0.99)
        assert len(results) < 3

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


class TestRouteDirectory:
    def test_descends_through_levels(self, tmp_path: Path) -> None:
        """Create a two-level tree and verify route descends."""
        from semtree.embedder import embed_directory, route_directory

        # Root level
        root_sem = tmp_path / ".sem"
        root_sem.mkdir()
        write_record(root_sem / "src.md", "src", "directory", "hash_src", "Source code with auth module.")
        write_record(root_sem / "docs.md", "docs", "directory", "hash_docs", "Documentation files.")
        write_record(root_sem / "__dir__.md", ".", "directory", "hash_root",
                     "Root.\n\n## Children\n\n- **src**: Source code.\n- **docs**: Docs.")

        # src level
        src_dir = tmp_path / "src"
        src_dir.mkdir()
        src_sem = src_dir / ".sem"
        src_sem.mkdir()
        write_record(src_sem / "auth.py.md", "src/auth.py", "file", "hash_auth", "Authentication module.")
        write_record(src_sem / "db.py.md", "src/db.py", "file", "hash_db", "Database layer.")
        write_record(src_sem / "__dir__.md", "src", "directory", "hash_src2",
                     "Source.\n\n## Children\n\n- **auth.py**: Auth.\n- **db.py**: Database.")

        # docs level (leaf)
        docs_dir = tmp_path / "docs"
        docs_dir.mkdir()

        # Embed everything
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=True)

        levels = route_directory(tmp_path, query="authentication login", model=DEFAULT_MODEL, beam_width=2)

        assert len(levels) >= 1
        # First level should have selected children
        assert len(levels[0]["selected"]) > 0
        # Each selected entry is (path, score, first_line)
        path, score, first_line = levels[0]["selected"][0]
        assert isinstance(score, float)
        assert isinstance(path, str)

    def test_respects_max_depth(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory, route_directory
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=True)

        levels = route_directory(tmp_path, query="anything", model=DEFAULT_MODEL, max_depth=1)
        # Should only descend 1 level (the root)
        assert len(levels) <= 1

    def test_beam_width_limits_selection(self, tmp_path: Path) -> None:
        _make_sem_tree(tmp_path)
        from semtree.embedder import embed_directory, route_directory
        embed_directory(tmp_path, model=DEFAULT_MODEL, force=True)

        levels = route_directory(tmp_path, query="login", model=DEFAULT_MODEL, beam_width=1)
        if levels:
            assert len(levels[0]["selected"]) <= 1

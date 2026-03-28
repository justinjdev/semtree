"""Tests for bench.quality module."""

from pathlib import Path

from bench.quality import run_quality_phase
from semtree.records import write_record


def _make_valid_tree(tmp_path: Path) -> None:
    """Create a minimal valid .sem/ tree."""
    sem = tmp_path / ".sem"
    sem.mkdir()
    write_record(sem / "foo.py.md", "foo.py", "file", "hash_foo", "Does foo things.")
    write_record(sem / "bar.py.md", "bar.py", "file", "hash_bar", "Does bar things.")
    write_record(
        sem / "__dir__.md", ".", "directory", "hash_dir",
        "Root.\n\n## Children\n\n- **foo.py**: Does foo things.\n- **bar.py**: Does bar things.",
    )
    # Create source files
    (tmp_path / "foo.py").write_text("# foo")
    (tmp_path / "bar.py").write_text("# bar")


class TestQualityPhase:
    def test_valid_tree_passes(self, tmp_path: Path) -> None:
        _make_valid_tree(tmp_path)
        records = run_quality_phase(tmp_path)
        metrics = {r.metric: r.value for r in records}
        assert metrics["children_coverage"] == 1.0
        assert metrics["frontmatter_errors"] == 0
        assert metrics["orphan_records"] == 0

    def test_missing_child_in_routing_table(self, tmp_path: Path) -> None:
        _make_valid_tree(tmp_path)
        # Add a file but don't mention it in __dir__.md children
        sem = tmp_path / ".sem"
        write_record(sem / "baz.py.md", "baz.py", "file", "hash_baz", "Does baz.")
        (tmp_path / "baz.py").write_text("# baz")

        records = run_quality_phase(tmp_path)
        metrics = {r.metric: r.value for r in records}
        assert metrics["children_coverage"] < 1.0

    def test_orphan_record_detected(self, tmp_path: Path) -> None:
        _make_valid_tree(tmp_path)
        # Create orphan: .sem record but no source file
        sem = tmp_path / ".sem"
        write_record(sem / "ghost.py.md", "ghost.py", "file", "hash_ghost", "Gone.")

        records = run_quality_phase(tmp_path)
        metrics = {r.metric: r.value for r in records}
        assert metrics["orphan_records"] >= 1

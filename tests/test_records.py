"""Tests for semtree.records module."""

from pathlib import Path

import pytest

from semtree.records import (
    read_record,
    record_path_for_dir,
    record_path_for_file,
    write_record,
)


class TestRecordPathForFile:
    def test_returns_sem_sibling(self, tmp_path: Path) -> None:
        result = record_path_for_file(tmp_path, "src/auth/login.py")
        assert result == tmp_path / "src" / "auth" / ".sem" / "login.py.md"

    def test_top_level_file(self, tmp_path: Path) -> None:
        result = record_path_for_file(tmp_path, "README.md")
        assert result == tmp_path / ".sem" / "README.md.md"


class TestRecordPathForDir:
    def test_returns_dir_record(self, tmp_path: Path) -> None:
        result = record_path_for_dir(tmp_path, "src/auth")
        assert result == tmp_path / "src" / "auth" / ".sem" / "__dir__.md"

    def test_empty_string_root(self, tmp_path: Path) -> None:
        result = record_path_for_dir(tmp_path, "")
        assert result == tmp_path / ".sem" / "__dir__.md"


class TestWriteRecord:
    def test_creates_sem_directory(self, tmp_path: Path) -> None:
        record_file = tmp_path / "pkg" / ".sem" / "mod.py.md"
        write_record(record_file, "pkg/mod.py", "file", "abc123", "A module.")

        assert record_file.parent.exists()
        assert record_file.parent.name == ".sem"

    def test_writes_yaml_frontmatter_and_body(self, tmp_path: Path) -> None:
        record_file = tmp_path / ".sem" / "app.py.md"
        write_record(record_file, "app.py", "file", "deadbeef", "Entry point.")

        text = record_file.read_text(encoding="utf-8")
        assert text.startswith("---\n")
        assert "path: app.py" in text
        assert "type: file" in text
        assert "content_hash: deadbeef" in text
        # Body appears after closing ---
        parts = text.split("---", 2)
        assert len(parts) == 3
        assert "Entry point." in parts[2]


class TestReadRecord:
    def test_returns_none_for_missing_file(self, tmp_path: Path) -> None:
        result = read_record(tmp_path / "nonexistent.md")
        assert result is None

    def test_parses_frontmatter_fields(self, tmp_path: Path) -> None:
        record_file = tmp_path / ".sem" / "foo.py.md"
        write_record(record_file, "foo.py", "file", "hash1", "Some summary.")

        data = read_record(record_file)
        assert data is not None
        assert data["path"] == "foo.py"
        assert data["type"] == "file"
        assert data["content_hash"] == "hash1"
        assert data["summary"] == "Some summary."

    def test_returns_none_for_malformed_file(self, tmp_path: Path) -> None:
        bad_file = tmp_path / "bad.md"
        bad_file.write_text("no frontmatter here", encoding="utf-8")
        assert read_record(bad_file) is None

    def test_returns_none_for_invalid_yaml(self, tmp_path: Path) -> None:
        bad_file = tmp_path / "bad.md"
        bad_file.write_text("---\n: :\n  - [\n---\nbody\n", encoding="utf-8")
        assert read_record(bad_file) is None


class TestRoundTrip:
    def test_write_then_read_consistent(self, tmp_path: Path) -> None:
        record_file = tmp_path / "src" / ".sem" / "lib.rs.md"
        write_record(
            record_file,
            path="src/lib.rs",
            node_type="file",
            content_hash="abc456",
            summary="Rust library root.",
        )

        data = read_record(record_file)
        assert data is not None
        assert data["path"] == "src/lib.rs"
        assert data["type"] == "file"
        assert data["content_hash"] == "abc456"
        assert data["summary"] == "Rust library root."

    def test_directory_record_round_trip(self, tmp_path: Path) -> None:
        record_file = record_path_for_dir(tmp_path, "src/auth")
        write_record(
            record_file,
            path="src/auth",
            node_type="directory",
            content_hash="dir999",
            summary="Authentication module.",
        )

        data = read_record(record_file)
        assert data is not None
        assert data["type"] == "directory"
        assert data["summary"] == "Authentication module."

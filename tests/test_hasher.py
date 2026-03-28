"""Tests for semtree.hasher module."""

import hashlib

import pytest

from semtree.hasher import hash_directory, hash_file


class TestHashFile:
    """Tests for hash_file()."""

    def test_known_hash(self, tmp_path):
        """SHA-256 of raw file bytes matches independently computed hash."""
        content = b"hello, semtree\n"
        f = tmp_path / "known.txt"
        f.write_bytes(content)

        expected = hashlib.sha256(content).hexdigest()
        assert hash_file(f) == expected

    def test_identical_files_produce_identical_hashes(self, tmp_path):
        """Two files with the same content produce the same hash."""
        content = b"duplicate content"
        a = tmp_path / "a.txt"
        b = tmp_path / "b.txt"
        a.write_bytes(content)
        b.write_bytes(content)

        assert hash_file(a) == hash_file(b)

    def test_different_files_produce_different_hashes(self, tmp_path):
        """Files with different content produce different hashes."""
        a = tmp_path / "a.txt"
        b = tmp_path / "b.txt"
        a.write_bytes(b"content A")
        b.write_bytes(b"content B")

        assert hash_file(a) != hash_file(b)

    def test_empty_file(self, tmp_path):
        """Empty file hashes to SHA-256 of empty bytes."""
        f = tmp_path / "empty.txt"
        f.write_bytes(b"")

        expected = hashlib.sha256(b"").hexdigest()
        assert hash_file(f) == expected


class TestHashDirectory:
    """Tests for hash_directory()."""

    def test_deterministic_from_sorted_children(self):
        """Sorted child pairs produce a deterministic hash."""
        children = [("src/a.py", "aaa"), ("src/b.py", "bbb")]
        h1 = hash_directory(children)
        h2 = hash_directory(children)
        assert h1 == h2

    def test_order_independence(self):
        """Different input order produces the same hash (internal sort)."""
        forward = [("src/a.py", "aaa"), ("src/b.py", "bbb")]
        reverse = [("src/b.py", "bbb"), ("src/a.py", "aaa")]
        assert hash_directory(forward) == hash_directory(reverse)

    def test_known_canonical_form(self):
        """Hash matches manually constructed canonical string."""
        children = [("z.py", "zzz"), ("a.py", "aaa")]
        canonical = "a.py:aaa\nz.py:zzz"
        expected = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
        assert hash_directory(children) == expected

    def test_different_children_produce_different_hashes(self):
        """Changing a child hash changes the directory hash."""
        original = [("a.py", "aaa"), ("b.py", "bbb")]
        modified = [("a.py", "aaa"), ("b.py", "ccc")]
        assert hash_directory(original) != hash_directory(modified)

    def test_empty_children(self):
        """Empty child list hashes to SHA-256 of empty string."""
        expected = hashlib.sha256(b"").hexdigest()
        assert hash_directory([]) == expected


class TestUpwardPropagation:
    """Changing a leaf file changes the parent directory hash."""

    def test_file_change_propagates_to_parent(self, tmp_path):
        """Modifying a file changes its hash and therefore the parent dir hash."""
        f = tmp_path / "module.py"
        f.write_bytes(b"version 1")

        file_hash_v1 = hash_file(f)
        dir_hash_v1 = hash_directory([("module.py", file_hash_v1)])

        f.write_bytes(b"version 2")

        file_hash_v2 = hash_file(f)
        dir_hash_v2 = hash_directory([("module.py", file_hash_v2)])

        assert file_hash_v1 != file_hash_v2
        assert dir_hash_v1 != dir_hash_v2

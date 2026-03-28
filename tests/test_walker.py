"""Tests for semtree.walker — post-order DFS filesystem traversal."""

import os
from pathlib import Path

import pytest

from semtree.walker import Node, walk, _is_binary


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _write(path: Path, content: str = "hello") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


def _names(nodes: list[Node]) -> list[str]:
    """Return repo-relative paths in order."""
    return [n.repo_relative_path for n in nodes]


# ---------------------------------------------------------------------------
# Post-order traversal
# ---------------------------------------------------------------------------

class TestPostOrder:
    def test_children_before_parent(self, tmp_path: Path) -> None:
        """Every child node must appear before its parent directory."""
        _write(tmp_path / "src" / "a.py")
        _write(tmp_path / "src" / "sub" / "b.py")

        nodes = walk(tmp_path)
        names = _names(nodes)

        # Files before their containing directory
        assert names.index("src/sub/b.py") < names.index("src/sub")
        assert names.index("src/sub") < names.index("src")
        assert names.index("src") < names.index("")

    def test_flat_directory(self, tmp_path: Path) -> None:
        """Files appear before the root directory node."""
        _write(tmp_path / "a.txt")
        _write(tmp_path / "b.txt")

        nodes = walk(tmp_path)
        names = _names(nodes)

        assert names[-1] == ""  # root is last
        assert "a.txt" in names
        assert "b.txt" in names


# ---------------------------------------------------------------------------
# Lexicographic sorting
# ---------------------------------------------------------------------------

class TestSorting:
    def test_files_sorted_within_directory(self, tmp_path: Path) -> None:
        _write(tmp_path / "z.py")
        _write(tmp_path / "a.py")
        _write(tmp_path / "m.py")

        nodes = walk(tmp_path)
        file_nodes = [n for n in nodes if not n.is_directory]
        assert _names(file_nodes) == ["a.py", "m.py", "z.py"]

    def test_subdirs_sorted(self, tmp_path: Path) -> None:
        """Subdirectory children in a parent's children list are sorted."""
        (tmp_path / "zulu").mkdir()
        _write(tmp_path / "zulu" / "f.txt")
        (tmp_path / "alpha").mkdir()
        _write(tmp_path / "alpha" / "f.txt")

        nodes = walk(tmp_path)
        root = [n for n in nodes if n.repo_relative_path == ""][0]
        assert root.children == ["alpha", "zulu"]

    def test_children_list_mixed_files_and_dirs(self, tmp_path: Path) -> None:
        """Children list includes both files and subdirs, all sorted."""
        _write(tmp_path / "z.py")
        _write(tmp_path / "a.py")
        (tmp_path / "mid").mkdir()
        _write(tmp_path / "mid" / "x.py")

        nodes = walk(tmp_path)
        root = [n for n in nodes if n.repo_relative_path == ""][0]
        assert root.children == ["a.py", "mid", "z.py"]


# ---------------------------------------------------------------------------
# Dotfile / dot-directory exclusion
# ---------------------------------------------------------------------------

class TestDotExclusion:
    def test_dotfiles_excluded(self, tmp_path: Path) -> None:
        _write(tmp_path / ".hidden")
        _write(tmp_path / "visible.py")

        nodes = walk(tmp_path)
        names = _names(nodes)
        assert ".hidden" not in names
        assert "visible.py" in names

    def test_dot_directories_excluded_from_parent_children(self, tmp_path: Path) -> None:
        """Dot-directories are excluded from the parent's children list."""
        _write(tmp_path / ".git" / "config")
        _write(tmp_path / "src" / "main.py")

        nodes = walk(tmp_path)
        root = [n for n in nodes if n.repo_relative_path == ""][0]
        # .git should not appear in root's children
        assert ".git" not in root.children
        assert "src" in root.children

    def test_nested_dotdir_excluded_from_parent_children(self, tmp_path: Path) -> None:
        """Dot-directories deeper in the tree are excluded from their parent's children."""
        _write(tmp_path / "pkg" / ".cache" / "data.bin", "text")
        _write(tmp_path / "pkg" / "mod.py")

        nodes = walk(tmp_path)
        pkg = [n for n in nodes if n.repo_relative_path == "pkg"][0]
        assert ".cache" not in [Path(c).name for c in pkg.children]
        assert "pkg/mod.py" in pkg.children


# ---------------------------------------------------------------------------
# Symlink exclusion
# ---------------------------------------------------------------------------

class TestSymlinkExclusion:
    def test_symlinked_file_excluded(self, tmp_path: Path) -> None:
        _write(tmp_path / "real.py")
        (tmp_path / "link.py").symlink_to(tmp_path / "real.py")

        nodes = walk(tmp_path)
        names = _names(nodes)
        assert "real.py" in names
        assert "link.py" not in names

    def test_symlinked_directory_excluded(self, tmp_path: Path) -> None:
        _write(tmp_path / "realdir" / "file.py")
        (tmp_path / "linkdir").symlink_to(tmp_path / "realdir")

        nodes = walk(tmp_path)
        names = _names(nodes)
        assert "realdir/file.py" in names
        assert not any("linkdir" in n for n in names)


# ---------------------------------------------------------------------------
# Binary file exclusion
# ---------------------------------------------------------------------------

class TestBinaryExclusion:
    def test_binary_file_excluded(self, tmp_path: Path) -> None:
        binary = tmp_path / "image.dat"
        binary.write_bytes(b"header\x00\x00rest of data")

        _write(tmp_path / "code.py")

        nodes = walk(tmp_path)
        names = _names(nodes)
        assert "code.py" in names
        assert "image.dat" not in names

    def test_text_file_not_excluded(self, tmp_path: Path) -> None:
        _write(tmp_path / "readme.txt", "no null bytes here")

        nodes = walk(tmp_path)
        file_nodes = [n for n in nodes if not n.is_directory]
        assert any(n.repo_relative_path == "readme.txt" for n in file_nodes)

    def test_is_binary_helper(self, tmp_path: Path) -> None:
        text_file = tmp_path / "text.py"
        text_file.write_text("print('hi')")
        assert _is_binary(text_file) is False

        bin_file = tmp_path / "bin.dat"
        bin_file.write_bytes(b"\x00" * 100)
        assert _is_binary(bin_file) is True

    def test_null_byte_after_8kb_not_detected(self, tmp_path: Path) -> None:
        """Null bytes past the first 8KB are not checked — file is text."""
        f = tmp_path / "sneaky.txt"
        f.write_bytes(b"a" * 8192 + b"\x00")

        assert _is_binary(f) is False


# ---------------------------------------------------------------------------
# Repo-relative path computation
# ---------------------------------------------------------------------------

class TestRepoRelativePath:
    def test_file_paths_are_relative(self, tmp_path: Path) -> None:
        _write(tmp_path / "src" / "lib" / "util.py")

        nodes = walk(tmp_path)
        util = [n for n in nodes if n.repo_relative_path == "src/lib/util.py"]
        assert len(util) == 1
        assert util[0].absolute_path == (tmp_path / "src" / "lib" / "util.py").resolve()

    def test_directory_paths_are_relative(self, tmp_path: Path) -> None:
        _write(tmp_path / "pkg" / "mod.py")

        nodes = walk(tmp_path)
        pkg = [n for n in nodes if n.repo_relative_path == "pkg"]
        assert len(pkg) == 1
        assert pkg[0].is_directory is True

    def test_root_node_empty_string(self, tmp_path: Path) -> None:
        """Root directory node has empty string as repo_relative_path."""
        _write(tmp_path / "file.py")

        nodes = walk(tmp_path)
        root = nodes[-1]
        assert root.repo_relative_path == ""
        assert root.is_directory is True
        assert root.absolute_path == tmp_path.resolve()

    def test_empty_repo_has_root_node(self, tmp_path: Path) -> None:
        """Even an empty directory produces a root node."""
        nodes = walk(tmp_path)
        assert len(nodes) == 1
        assert nodes[0].repo_relative_path == ""
        assert nodes[0].is_directory is True
        assert nodes[0].children == []

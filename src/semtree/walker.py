"""Filesystem traversal for SRT construction.

Post-order DFS: children are yielded before their parent directory.
"""

import os
from dataclasses import dataclass
from pathlib import Path


SEM_DIR = ".sem"


@dataclass
class Node:
    repo_relative_path: str
    absolute_path: Path
    is_directory: bool
    children: list[str]  # repo-relative paths of immediate children


def _is_binary(path: Path) -> bool:
    """Check if a file is binary by looking for null bytes in the first 8KB."""
    try:
        with open(path, "rb") as f:
            chunk = f.read(8192)
        return b"\x00" in chunk
    except OSError:
        return True


def _should_skip_entry(name: str, full_path: Path) -> bool:
    """Return True if this entry should be excluded from the tree."""
    if name.startswith("."):
        return True
    if os.path.islink(full_path):
        return True
    return False


def walk(root: Path) -> list[Node]:
    """Walk the repository in post-order DFS, yielding nodes bottom-up.

    Returns a list of Node objects where children always appear before
    their parent directory.

    Uses topdown=True so we can prune dot-directories before os.walk
    descends into them, then reverses directory ordering to achieve
    post-order (children before parents).
    """
    root = root.resolve()

    # Collect directory info top-down (so we can prune), then reverse
    dir_entries: list[tuple[Path, list[str], list[str]]] = []

    for dirpath_str, dirnames, filenames in os.walk(root, topdown=True):
        dirpath = Path(dirpath_str)

        # Filter dirnames in-place — this prevents os.walk from
        # descending into dot-directories, symlinked dirs, etc.
        dirnames[:] = sorted(
            d for d in dirnames
            if not _should_skip_entry(d, dirpath / d)
        )

        dir_entries.append((dirpath, list(dirnames), filenames))

    # Reverse to get post-order: deepest directories first
    dir_entries.reverse()

    nodes: list[Node] = []
    for dirpath, subdirs, filenames in dir_entries:
        child_paths: list[str] = []

        # Process files in sorted order
        for fname in sorted(filenames):
            fpath = dirpath / fname
            if _should_skip_entry(fname, fpath):
                continue
            if _is_binary(fpath):
                continue

            rel = str(fpath.relative_to(root))
            child_paths.append(rel)
            nodes.append(Node(
                repo_relative_path=rel,
                absolute_path=fpath,
                is_directory=False,
                children=[],
            ))

        # Add subdirectory children
        for dname in subdirs:
            rel = str((dirpath / dname).relative_to(root))
            child_paths.append(rel)

        # Emit directory node
        rel_dir = str(dirpath.relative_to(root))
        if rel_dir == ".":
            rel_dir = ""

        nodes.append(Node(
            repo_relative_path=rel_dir,
            absolute_path=dirpath,
            is_directory=True,
            children=sorted(child_paths),
        ))

    return nodes

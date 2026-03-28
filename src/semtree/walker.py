"""Filesystem traversal for SRT construction.

Post-order DFS: children are yielded before their parent directory.
Uses `git ls-files` when available to respect .gitignore and skip untracked files.
Falls back to filesystem walk for non-git repos.
"""

import fnmatch
import os
import subprocess
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


SEM_DIR = ".sem"

# Directories always excluded (build artifacts, deps, caches)
DEFAULT_EXCLUDE_DIRS = {
    "node_modules", "vendor", "dist", "build", "target",
    "third_party", "__pycache__", ".build", ".gradle",
    "_build", "deps",  # elixir
    "_app", "immutable",  # SvelteKit/Vite build output
}

# File patterns always excluded (generated code, lock files, build output)
DEFAULT_EXCLUDE_SUFFIXES = (
    ".lock", ".sum",              # lock files
    ".min.js", ".min.css",        # minified
    ".bundle.js", ".chunk.js",    # bundled
    ".generated.go", "_generated.go", ".pb.go", ".gen.go",  # go generated
    ".generated.ts", ".generated.js",                        # ts/js generated
    "_pb2.py", "_pb2_grpc.py",    # python protobuf
    ".pb.cc", ".pb.h", ".grpc.pb.cc", ".grpc.pb.h",         # c++ protobuf
    ".d.ts",                      # type declarations
)

DEFAULT_EXCLUDE_FILES = {
    "package-lock.json", "pnpm-lock.yaml", "yarn.lock",
    "Cargo.lock", "Gemfile.lock", "poetry.lock", "composer.lock",
    "go.sum",
}


@dataclass
class Node:
    repo_relative_path: str
    absolute_path: Path
    is_directory: bool
    children: list[str]  # repo-relative paths of immediate children


def _matches_exclude(rel_path: str, patterns: tuple[str, ...]) -> bool:
    """Check if a repo-relative path matches any exclude glob pattern."""
    for pattern in patterns:
        if fnmatch.fnmatch(rel_path, pattern):
            return True
        # Also check if any parent directory matches
        parts = Path(rel_path).parts
        for i in range(len(parts)):
            partial = str(Path(*parts[: i + 1]))
            if fnmatch.fnmatch(partial, pattern):
                return True
    return False


def _should_skip_file(rel_path: str) -> bool:
    """Check if a file should be skipped based on default exclusion rules."""
    name = Path(rel_path).name
    if name in DEFAULT_EXCLUDE_FILES:
        return True
    for suffix in DEFAULT_EXCLUDE_SUFFIXES:
        if name.endswith(suffix):
            return True
    return False


def _should_skip_dir(name: str) -> bool:
    """Check if a directory should be skipped based on default exclusion rules."""
    return name in DEFAULT_EXCLUDE_DIRS


def _is_binary(path: Path) -> bool:
    """Check if a file is binary by looking for null bytes in the first 8KB."""
    try:
        with open(path, "rb") as f:
            chunk = f.read(8192)
        return b"\x00" in chunk
    except OSError:
        return True


def _git_tracked_files(root: Path) -> list[str] | None:
    """Return sorted list of git-tracked file paths, or None if not a git repo."""
    try:
        result = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode != 0:
            return None
        return sorted(line for line in result.stdout.splitlines() if line)
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None


def _build_tree_from_git(root: Path, tracked_files: list[str], exclude: tuple[str, ...] = ()) -> list[Node]:
    """Build post-order node list from git-tracked files.

    Groups files by directory, builds directory nodes bottom-up.
    """
    root = root.resolve()

    # Filter out binary files, dotfiles, generated code, and excluded patterns
    valid_files: list[str] = []
    for rel in tracked_files:
        parts = Path(rel).parts
        if any(p.startswith(".") for p in parts):
            continue
        if any(_should_skip_dir(p) for p in parts[:-1]):
            continue
        if _should_skip_file(rel):
            continue
        if exclude and _matches_exclude(rel, exclude):
            continue
        fpath = root / rel
        if not fpath.is_file():
            continue
        if _is_binary(fpath):
            continue
        valid_files.append(rel)

    # Build directory -> immediate children mapping
    dir_children: dict[str, set[str]] = defaultdict(set)

    for rel in valid_files:
        # Register file as child of its parent dir
        parent = str(Path(rel).parent)
        if parent == ".":
            parent = ""
        dir_children[parent].add(rel)

        # Register all ancestor directories
        parts = Path(rel).parts
        for i in range(len(parts) - 1):
            dir_path = str(Path(*parts[:i + 1]))
            parent_dir = str(Path(*parts[:i])) if i > 0 else ""
            dir_children[parent_dir].add(dir_path)

    # Collect all directories (including root)
    all_dirs = set(dir_children.keys())
    for children in dir_children.values():
        for c in children:
            if c in dir_children or (root / c).is_dir():
                all_dirs.add(c)

    # Sort directories deepest-first for post-order
    sorted_dirs = sorted(all_dirs, key=lambda d: (-d.count(os.sep) if d else 1, d))

    nodes: list[Node] = []
    for dir_path in sorted_dirs:
        children = sorted(dir_children.get(dir_path, set()))

        # Emit file nodes first (sorted)
        file_children = [c for c in children if c in valid_files]
        dir_child_paths = [c for c in children if c not in valid_files]

        for rel in sorted(file_children):
            nodes.append(Node(
                repo_relative_path=rel,
                absolute_path=root / rel,
                is_directory=False,
                children=[],
            ))

        # Emit directory node
        nodes.append(Node(
            repo_relative_path=dir_path,
            absolute_path=root / dir_path if dir_path else root,
            is_directory=True,
            children=sorted(file_children + dir_child_paths),
        ))

    return nodes


def _build_tree_from_fs(root: Path, exclude: tuple[str, ...] = ()) -> list[Node]:
    """Build post-order node list from filesystem walk (non-git fallback)."""
    root = root.resolve()
    dir_entries: list[tuple[Path, list[str], list[str]]] = []

    for dirpath_str, dirnames, filenames in os.walk(root, topdown=True):
        dirpath = Path(dirpath_str)

        dirnames[:] = sorted(
            d for d in dirnames
            if not d.startswith(".")
            and not os.path.islink(dirpath / d)
            and not _should_skip_dir(d)
            and not (exclude and _matches_exclude(str((dirpath / d).relative_to(root)), exclude))
        )
        dir_entries.append((dirpath, list(dirnames), filenames))

    dir_entries.reverse()

    nodes: list[Node] = []
    for dirpath, subdirs, filenames in dir_entries:
        child_paths: list[str] = []

        for fname in sorted(filenames):
            fpath = dirpath / fname
            if fname.startswith(".") or os.path.islink(fpath):
                continue
            rel = str(fpath.relative_to(root))
            if _should_skip_file(rel):
                continue
            if exclude and _matches_exclude(rel, exclude):
                continue
            if _is_binary(fpath):
                continue
            child_paths.append(rel)
            nodes.append(Node(
                repo_relative_path=rel,
                absolute_path=fpath,
                is_directory=False,
                children=[],
            ))

        for dname in subdirs:
            rel = str((dirpath / dname).relative_to(root))
            child_paths.append(rel)

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


def walk(root: Path, exclude: tuple[str, ...] = ()) -> list[Node]:
    """Walk the repository in post-order DFS, yielding nodes bottom-up.

    Uses git ls-files when in a git repo (respects .gitignore).
    Falls back to filesystem walk otherwise.
    """
    root = root.resolve()
    tracked = _git_tracked_files(root)
    if tracked is not None:
        return _build_tree_from_git(root, tracked, exclude)
    return _build_tree_from_fs(root, exclude)

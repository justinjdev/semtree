"""Content hashing for SRT nodes.

Files: SHA-256 of raw bytes.
Directories: SHA-256 of sorted (child_path:child_hash) pairs joined by newlines.
"""

import hashlib
from pathlib import Path


def hash_file(path: Path) -> str:
    """Compute SHA-256 hex digest of a file's raw byte contents."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def hash_directory(children: list[tuple[str, str]]) -> str:
    """Compute SHA-256 hex digest from sorted (path, hash) child pairs.

    The canonical string is formed by sorting children lexicographically
    by path, formatting each as 'path:hash', and joining with newlines.
    """
    sorted_children = sorted(children, key=lambda c: c[0])
    canonical = "\n".join(f"{path}:{h}" for path, h in sorted_children)
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()

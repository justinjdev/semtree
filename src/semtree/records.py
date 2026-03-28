"""Read and write .sem/ summary records.

Each record is a Markdown file with YAML frontmatter:
---
path: <repo-relative-path>
type: file|directory
content_hash: <sha256-hex>
---

<summary body>
"""

from pathlib import Path
from typing import Any

import yaml


SEM_DIR = ".sem"
DIR_RECORD = "__dir__.md"


def record_path_for_file(repo_root: Path, repo_relative: str) -> Path:
    """Return the .sem/ record path for a file node."""
    source = repo_root / repo_relative
    return source.parent / SEM_DIR / f"{source.name}.md"


def record_path_for_dir(repo_root: Path, repo_relative: str) -> Path:
    """Return the .sem/ record path for a directory node."""
    if repo_relative == "":
        return repo_root / SEM_DIR / DIR_RECORD
    return repo_root / repo_relative / SEM_DIR / DIR_RECORD


def write_record(
    record_file: Path,
    path: str,
    node_type: str,
    content_hash: str,
    summary: str,
) -> None:
    """Write a .sem/ record with YAML frontmatter and Markdown body."""
    record_file.parent.mkdir(parents=True, exist_ok=True)

    frontmatter = {
        "path": path,
        "type": node_type,
        "content_hash": content_hash,
    }
    fm_str = yaml.dump(frontmatter, default_flow_style=False, sort_keys=False).rstrip()

    content = f"---\n{fm_str}\n---\n\n{summary}\n"
    record_file.write_text(content, encoding="utf-8")


def read_record(record_file: Path) -> dict[str, Any] | None:
    """Read a .sem/ record and return parsed frontmatter, or None if missing."""
    if not record_file.exists():
        return None

    text = record_file.read_text(encoding="utf-8")
    parts = text.split("---", 2)
    if len(parts) < 3:
        return None

    try:
        fm = yaml.safe_load(parts[1])
    except yaml.YAMLError:
        return None

    if not isinstance(fm, dict):
        return None

    fm["summary"] = parts[2].strip()
    return fm

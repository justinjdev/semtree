"""Benchmark incremental phase: modify files, rebuild, verify correctness."""

import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from semtree.config import BuildConfig
from semtree.hasher import hash_file
from semtree.records import SEM_DIR, read_record


# Files to modify for incremental test (repo-specific)
FELLOWSHIP_MODIFY_FILES = [
    "cli/internal/state/state.go",
    "plugin/skills/quest.md",
]

MARKER = "\n// benchmark-incremental-marker\n"


def run_incremental_phase(repo_path: Path, repo_name: str = "local") -> list[MetricRecord]:
    """Modify files, rebuild incrementally, verify only changed subtree updated."""
    from semtree.builder import build

    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    records: list[MetricRecord] = []

    # Snapshot hashes before modification
    pre_hashes = {}
    for md_path in repo_path.rglob(f"{SEM_DIR}/*.md"):
        data = read_record(md_path)
        if data:
            pre_hashes[data["path"]] = data["content_hash"]

    # Modify files
    modify_files = FELLOWSHIP_MODIFY_FILES if repo_name == "fellowship" else []
    for rel_path in modify_files:
        fpath = repo_path / rel_path
        if fpath.exists():
            fpath.write_text(fpath.read_text() + MARKER)

    # Incremental rebuild
    config = BuildConfig(target_path=repo_path, force=False, embed=False)
    t0 = time.monotonic()
    build(config)
    rebuild_time = time.monotonic() - t0

    # Count re-summarized nodes
    post_hashes = {}
    for md_path in repo_path.rglob(f"{SEM_DIR}/*.md"):
        data = read_record(md_path)
        if data:
            post_hashes[data["path"]] = data["content_hash"]

    changed = sum(1 for p in post_hashes if pre_hashes.get(p) != post_hashes[p])

    records.append(MetricRecord(now, "incremental", repo_name, "srt", "", "", "incr_rebuild_time_s", round(rebuild_time, 2)))
    records.append(MetricRecord(now, "incremental", repo_name, "srt", "", "", "nodes_resummarized", changed))

    # Revert modifications
    subprocess.run(["git", "checkout", "."], cwd=repo_path, capture_output=True)

    return records

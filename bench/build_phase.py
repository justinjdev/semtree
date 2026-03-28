"""Benchmark build phase: measure full and incremental build cost."""

import shutil
import time
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from semtree.config import BuildConfig
from semtree.records import SEM_DIR


def run_build_phase(repo_path: Path, repo_name: str = "local") -> list[MetricRecord]:
    """Run full build, then incremental no-op build. Returns metric records."""
    from semtree.builder import build

    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    records: list[MetricRecord] = []

    # Clean existing .sem/ dirs for fresh build
    for sem_dir in list(repo_path.rglob(SEM_DIR)):
        if sem_dir.is_dir():
            shutil.rmtree(sem_dir)

    # Full build
    config = BuildConfig(target_path=repo_path, force=True, embed=False)
    t0 = time.monotonic()
    build(config)
    build_time = time.monotonic() - t0

    # Count nodes by counting .sem/*.md files
    node_count = len(list(repo_path.rglob(f"{SEM_DIR}/*.md")))

    records.append(MetricRecord(now, "build", repo_name, "srt", "", "", "build_time_s", round(build_time, 2)))
    records.append(MetricRecord(now, "build", repo_name, "srt", "", "", "node_count", node_count))

    # Incremental no-op build
    config_incr = BuildConfig(target_path=repo_path, force=False, embed=False)
    t0 = time.monotonic()
    build(config_incr)
    incr_time = time.monotonic() - t0

    records.append(MetricRecord(now, "build", repo_name, "srt", "", "", "incr_build_time_s", round(incr_time, 2)))

    return records

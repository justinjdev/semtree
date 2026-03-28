"""Benchmark quality phase: structural correctness checks on .sem/ records."""

import re
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from semtree.records import SEM_DIR, read_record


def run_quality_phase(repo_path: Path, repo_name: str = "local") -> list[MetricRecord]:
    """Run structural quality checks on all .sem/ records."""
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    records: list[MetricRecord] = []

    all_sem_records = list(repo_path.rglob(f"{SEM_DIR}/*.md"))
    frontmatter_errors = 0
    orphan_count = 0
    coverage_scores = []

    for md_path in all_sem_records:
        data = read_record(md_path)
        if data is None:
            frontmatter_errors += 1
            continue

        # Frontmatter validity
        for field in ("path", "type", "content_hash"):
            if field not in data:
                frontmatter_errors += 1
                break
        if data.get("type") not in ("file", "directory"):
            frontmatter_errors += 1

        # Orphan check: does the source exist?
        rel_path = data.get("path", "")
        if data.get("type") == "file":
            source = repo_path / rel_path
            if not source.exists():
                orphan_count += 1
        elif data.get("type") == "directory":
            dir_path = repo_path / rel_path if rel_path and rel_path != "." else repo_path
            if not dir_path.is_dir():
                orphan_count += 1

        # Children coverage (for directory records)
        if data.get("type") == "directory":
            summary = data.get("summary", "")
            mentioned = set(re.findall(r"\*\*([^*]+)\*\*", summary))
            dir_path = repo_path / rel_path if rel_path and rel_path != "." else repo_path
            sem_dir = dir_path / SEM_DIR
            if sem_dir.is_dir():
                child_records = [
                    p.stem.replace(".md", "") if p.name != "__dir__.md" else None
                    for p in sem_dir.glob("*.md")
                ]
                child_names = {c for c in child_records if c is not None}
                if child_names:
                    found = sum(1 for c in child_names if c in mentioned)
                    coverage_scores.append(found / len(child_names))

    avg_coverage = sum(coverage_scores) / len(coverage_scores) if coverage_scores else 1.0

    records.append(MetricRecord(now, "quality", repo_name, "srt", "", "", "children_coverage", avg_coverage))
    records.append(MetricRecord(now, "quality", repo_name, "srt", "", "", "frontmatter_errors", frontmatter_errors))
    records.append(MetricRecord(now, "quality", repo_name, "srt", "", "", "orphan_records", orphan_count))

    return records

"""Benchmark routing phase: simulated SRT descent with control grid."""

import json
import math
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

import yaml

from bench.harness import MetricRecord
from semtree.records import SEM_DIR, DIR_RECORD, read_record


@dataclass
class Query:
    id: str
    question: str
    category: str
    relevant: list[dict]  # [{"path": str, "relevance": int}]


@dataclass
class DescentResult:
    files_reached: list[str]
    llm_calls: int
    tokens_loaded: int
    elapsed_s: float


def load_queries(query_file: Path) -> list[Query]:
    """Load query set from YAML file."""
    data = yaml.safe_load(query_file.read_text(encoding="utf-8"))
    return [
        Query(
            id=q["id"],
            question=q["question"],
            category=q["category"],
            relevant=q.get("relevant", []),
        )
        for q in data["queries"]
    ]


def ndcg_at_k(retrieved: list[str], relevant: dict[str, int], k: int = 10) -> float:
    """Compute NDCG@k with graded relevance."""
    if not relevant:
        return 0.0

    # DCG of retrieved
    dcg = 0.0
    for i, path in enumerate(retrieved[:k]):
        rel = relevant.get(path, 0)
        dcg += (2 ** rel - 1) / math.log2(i + 2)

    # Ideal DCG: sort relevant by relevance desc
    ideal_rels = sorted(relevant.values(), reverse=True)[:k]
    idcg = 0.0
    for i, rel in enumerate(ideal_rels):
        idcg += (2 ** rel - 1) / math.log2(i + 2)

    if idcg == 0:
        return 0.0
    return dcg / idcg


SelectFn = Callable[[str, list[tuple[str, str]], int], list[str]]


def simulate_descent(
    repo_path: Path,
    question: str,
    select_fn: SelectFn,
    beam_width: int = 3,
    max_depth: int = 10,
    token_budget: int = 50000,
) -> DescentResult:
    """Simulate SRT routing protocol descent.

    select_fn(question, [(child_path, child_summary)], beam_width) -> [selected_paths]
    """
    files_reached = []
    llm_calls = 0
    tokens_loaded = 0
    t0 = time.monotonic()

    # Start at root
    queue = [("", 0)]  # (dir_relative_path, depth)

    while queue and tokens_loaded < token_budget:
        rel_path, depth = queue.pop(0)
        if depth > max_depth:
            continue

        # Read directory record
        if rel_path == "":
            dir_record_path = repo_path / SEM_DIR / DIR_RECORD
        else:
            dir_record_path = repo_path / rel_path / SEM_DIR / DIR_RECORD

        data = read_record(dir_record_path)
        if data is None:
            continue

        summary = data.get("summary", "")
        tokens_loaded += len(summary) // 4  # rough token estimate

        # Extract children from summary
        children = _extract_children(repo_path, rel_path)
        if not children:
            continue

        # LLM selects children
        selected = select_fn(question, children, beam_width)
        llm_calls += 1

        for child_path in selected:
            child_full = repo_path / child_path
            if child_full.is_dir():
                queue.append((child_path, depth + 1))
            else:
                files_reached.append(child_path)
                # Load file summary tokens
                sem_dir = child_full.parent / SEM_DIR
                file_record = sem_dir / f"{child_full.name}.md"
                file_data = read_record(file_record)
                if file_data:
                    tokens_loaded += len(file_data.get("summary", "")) // 4

    elapsed = time.monotonic() - t0
    return DescentResult(
        files_reached=files_reached,
        llm_calls=llm_calls,
        tokens_loaded=tokens_loaded,
        elapsed_s=elapsed,
    )


def _extract_children(repo_path: Path, dir_rel_path: str) -> list[tuple[str, str]]:
    """Extract (child_path, child_summary) pairs from .sem/ records."""
    if dir_rel_path == "":
        sem_dir = repo_path / SEM_DIR
    else:
        sem_dir = repo_path / dir_rel_path / SEM_DIR

    if not sem_dir.is_dir():
        return []

    children = []
    for md_path in sorted(sem_dir.glob("*.md")):
        if md_path.name == DIR_RECORD:
            continue
        data = read_record(md_path)
        if data:
            children.append((data["path"], data.get("summary", "")))
    return children


# Control grid for SRT
SRT_CONTROL_GRID = [
    {"beam_width": bw, "max_depth": md, "token_budget": tb}
    for bw in [1, 2, 3, 5]
    for md in [1, 2, 3, 100]  # 100 = unlimited
    for tb in [1000, 2000, 5000, 10000, 20000, 50000]
]

# Per-call cost estimate (Claude Haiku for routing)
COST_PER_LLM_CALL = 0.001  # $0.001 per call estimate


def run_routing_phase(
    repo_path: Path,
    query_file: Path,
    select_fn: SelectFn,
    repo_name: str = "local",
) -> list[MetricRecord]:
    """Run routing phase: sweep control grid, collect metrics per query per setting."""
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    queries = load_queries(query_file)
    records: list[MetricRecord] = []

    for query in queries:
        relevant_map = {r["path"]: r["relevance"] for r in query.relevant}

        for control in SRT_CONTROL_GRID:
            control_json = json.dumps(control, sort_keys=True)

            result = simulate_descent(
                repo_path=repo_path,
                question=query.question,
                select_fn=select_fn,
                beam_width=control["beam_width"],
                max_depth=control["max_depth"],
                token_budget=control["token_budget"],
            )

            ndcg = ndcg_at_k(result.files_reached, relevant_map, k=10)
            cost = result.llm_calls * COST_PER_LLM_CALL

            for metric, value in [
                ("ndcg@10", ndcg),
                ("cost_usd", cost),
                ("latency_s", result.elapsed_s),
                ("tokens_loaded", result.tokens_loaded),
                ("llm_calls", result.llm_calls),
            ]:
                records.append(MetricRecord(
                    now, "routing", repo_name, "srt",
                    query.id, control_json, metric, value,
                ))

    return records

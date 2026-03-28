"""Grep/glob baseline: simulates agent search without SRT summaries."""

import json
import re
import subprocess
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from bench.harness import MetricRecord
from bench.routing import Query, load_queries, ndcg_at_k


@dataclass
class BaselineResult:
    files_found: list[str]
    tokens_loaded: int
    elapsed_s: float


def _extract_keywords(question: str) -> list[str]:
    """Extract search keywords from a question (simple heuristic, no LLM)."""
    stop_words = {"how", "does", "the", "a", "an", "is", "what", "where", "which", "when", "do", "are", "in", "of", "to", "for", "and", "or", "with"}
    words = re.findall(r"[a-zA-Z_]+", question.lower())
    return [w for w in words if w not in stop_words and len(w) > 2]


def grep_search(
    repo_path: Path,
    question: str,
    max_files: int = 5,
    strategy: str = "grep_only",
) -> BaselineResult:
    """Search repo using grep/glob, return found files."""
    t0 = time.monotonic()
    keywords = _extract_keywords(question)
    found_files: dict[str, int] = {}  # path -> match count

    for keyword in keywords:
        result = subprocess.run(
            ["grep", "-rl",
             "--include=*.go", "--include=*.py", "--include=*.md",
             "--include=*.ts", "--include=*.js",
             "--exclude-dir=node_modules", "--exclude-dir=.git",
             "--exclude-dir=.sem", "--exclude-dir=.srt",
             "--exclude-dir=.shire", "--exclude-dir=vendor",
             keyword, "."],
            cwd=repo_path, capture_output=True, text=True,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().split("\n"):
                path = line.lstrip("./")
                if path and not path.startswith(".sem/"):
                    found_files[path] = found_files.get(path, 0) + 1

    # Rank by match count, take top max_files
    ranked = sorted(found_files.items(), key=lambda x: x[1], reverse=True)
    top_files = [path for path, _ in ranked[:max_files]]

    # Estimate tokens loaded (read file sizes)
    tokens = 0
    for f in top_files:
        fpath = repo_path / f
        if fpath.exists():
            tokens += fpath.stat().st_size // 4

    elapsed = time.monotonic() - t0
    return BaselineResult(files_found=top_files, tokens_loaded=tokens, elapsed_s=elapsed)


# Control grid for baseline
BASELINE_CONTROL_GRID = [
    {"max_files": mf, "strategy": strat, "token_budget": tb}
    for mf in [3, 5, 10, 20]
    for strat in ["grep_only", "glob_then_grep"]
    for tb in [1000, 2000, 5000, 10000, 20000, 50000]
]


def run_baseline_phase(
    repo_path: Path,
    query_file: Path,
    repo_name: str = "local",
    results_path: Path | None = None,
) -> list[MetricRecord]:
    """Run baseline phase: sweep control grid, collect metrics.

    If results_path is provided, writes results incrementally after each test.
    """
    from bench.harness import append_results

    queries = load_queries(query_file)
    records: list[MetricRecord] = []

    for query in queries:
        relevant_map = {r["path"]: r["relevance"] for r in query.relevant}

        for control in BASELINE_CONTROL_GRID:
            now = datetime.now(timezone.utc).isoformat(timespec="seconds")
            control_json = json.dumps(control, sort_keys=True)

            result = grep_search(
                repo_path=repo_path,
                question=query.question,
                max_files=control["max_files"],
                strategy=control["strategy"],
            )

            # Truncate by token budget
            files_within_budget = []
            token_sum = 0
            for f in result.files_found:
                fsize = (repo_path / f).stat().st_size // 4 if (repo_path / f).exists() else 0
                if token_sum + fsize <= control["token_budget"]:
                    files_within_budget.append(f)
                    token_sum += fsize

            ndcg = ndcg_at_k(files_within_budget, relevant_map, k=10)

            batch: list[MetricRecord] = []
            for metric, value in [
                ("ndcg@10", ndcg),
                ("cost_usd", 0.0),  # grep is free
                ("latency_s", result.elapsed_s),
                ("tokens_loaded", token_sum),
                ("llm_calls", 0),
            ]:
                batch.append(MetricRecord(
                    now, "routing", repo_name, "baseline",
                    query.id, control_json, metric, value,
                ))

            records.extend(batch)
            if results_path:
                append_results(results_path, batch)

    return records

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
class LevelTelemetry:
    depth: int
    n_candidates: int
    n_selected: int
    selected_paths: list[str]
    rho_l: float = 0.0  # irrelevant fraction, computed post-hoc


@dataclass
class DescentResult:
    files_reached: list[str]
    llm_calls: int
    tokens_loaded: int
    elapsed_s: float
    level_telemetry: list[LevelTelemetry] = field(default_factory=list)


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


def precision(retrieved: list[str], relevant_set: set[str]) -> float:
    """Precision: fraction of retrieved items that are relevant."""
    if not retrieved:
        return 0.0
    return sum(1 for r in retrieved if r in relevant_set) / len(retrieved)


def recall(retrieved: list[str], relevant_set: set[str]) -> float:
    """Recall: fraction of relevant items that were retrieved."""
    if not relevant_set:
        return 0.0
    return sum(1 for r in retrieved if r in relevant_set) / len(relevant_set)


def mrr(retrieved: list[str], relevant_set: set[str]) -> float:
    """Mean Reciprocal Rank: 1/rank of first relevant item."""
    for i, r in enumerate(retrieved):
        if r in relevant_set:
            return 1.0 / (i + 1)
    return 0.0


def compute_rho_l(selected_paths: list[str], relevant_paths: set[str]) -> float:
    """Compute irrelevant fraction rho_l.

    A selected path is "relevant" if it is an ancestor of any relevant leaf
    or is a relevant leaf itself.
    """
    if not selected_paths:
        return 0.0

    def is_on_relevant_path(path: str) -> bool:
        for rp in relevant_paths:
            if rp == path or rp.startswith(path.rstrip("/") + "/"):
                return True
        return False

    irrelevant = sum(1 for p in selected_paths if not is_on_relevant_path(p))
    return irrelevant / len(selected_paths)


def log_dilution_penalty(
    telemetry: list[LevelTelemetry],
    weights: list[float] | None = None,
) -> float:
    """D(b,d) = sum w_l * log(1 + n_selected_l)"""
    if not telemetry:
        return 0.0
    w = weights or [1.0] * len(telemetry)
    if len(w) < len(telemetry):
        raise ValueError(f"weights length {len(w)} < telemetry length {len(telemetry)}")
    return sum(w[i] * math.log(1 + t.n_selected) for i, t in enumerate(telemetry))


def ratio_dilution_penalty(
    telemetry: list[LevelTelemetry],
    weights: list[float] | None = None,
) -> float:
    """D'(b,d) = sum w_l * rho_l"""
    if not telemetry:
        return 0.0
    w = weights or [1.0] * len(telemetry)
    if len(w) < len(telemetry):
        raise ValueError(f"weights length {len(w)} < telemetry length {len(telemetry)}")
    return sum(w[i] * t.rho_l for i, t in enumerate(telemetry))


SelectFn = Callable[[str, list[tuple[str, str]], int], list[tuple[str, float]]]


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

    Files are ranked by the depth at which they were discovered (deeper = more
    specifically routed), with ties broken by selection order within a level.
    """
    # Track files with their selection score
    files_with_score: list[tuple[str, float]] = []
    llm_calls = 0
    tokens_loaded = 0
    level_telemetry: list[LevelTelemetry] = []
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

        # Select children (returns (path, score) pairs)
        selected = select_fn(question, children, beam_width)
        llm_calls += 1

        selected_paths = [p for p, _ in selected]
        level_telemetry.append(LevelTelemetry(
            depth=depth,
            n_candidates=len(children),
            n_selected=len(selected),
            selected_paths=selected_paths,
        ))

        for child_path, score in selected:
            child_full = repo_path / child_path
            if child_full.is_dir():
                queue.append((child_path, depth + 1))
            else:
                files_with_score.append((child_path, score))
                # Load file summary tokens
                sem_dir = child_full.parent / SEM_DIR
                file_record = sem_dir / f"{child_full.name}.md"
                file_data = read_record(file_record)
                if file_data:
                    tokens_loaded += len(file_data.get("summary", "")) // 4

    # Rank files by cosine similarity score (highest = most relevant)
    files_with_score.sort(key=lambda x: x[1], reverse=True)
    files_reached = [path for path, _score in files_with_score]

    elapsed = time.monotonic() - t0
    return DescentResult(
        files_reached=files_reached,
        llm_calls=llm_calls,
        tokens_loaded=tokens_loaded,
        elapsed_s=elapsed,
        level_telemetry=level_telemetry,
    )


def enrich_telemetry(telemetry: list[LevelTelemetry], relevant_paths: set[str]) -> None:
    """Compute rho_l for each level given ground truth relevant paths."""
    for t in telemetry:
        t.rho_l = compute_rho_l(t.selected_paths, relevant_paths)


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

# Embedding cost: model load is ~0.3s amortized, per-query embed is ~0.002s
COST_PER_EMBED_CALL = 0.0  # local inference, no API cost


def make_embedding_select_fn(repo_path: Path, model: str = "BAAI/bge-small-en-v1.5") -> SelectFn:
    """Create a select_fn that uses cosine similarity over .vec sidecars.

    Loads the embedding model once; subsequent calls just embed the query and rank.
    """
    from semtree.embedder import embed_query, read_vec, cosine_rank

    # Cache query embeddings across calls within a run
    _query_cache: dict[str, list[float]] = {}

    def select_fn(question: str, children: list[tuple[str, str]], beam_width: int) -> list[tuple[str, float]]:
        if question not in _query_cache:
            _query_cache[question] = embed_query(question, model_name=model)
        query_vec = _query_cache[question]

        # Load child vectors
        children_vecs: dict[str, list[float]] = {}
        for child_path, _summary in children:
            child_full = repo_path / child_path
            vec_path = child_full.parent / ".sem" / f"{child_full.name}.vec"
            vec_data = read_vec(vec_path)
            if vec_data is not None:
                children_vecs[child_path] = vec_data["vector"]

        if not children_vecs:
            return [(path, 0.0) for path, _ in children[:beam_width]]

        ranked = cosine_rank(query_vec, children_vecs)
        return ranked[:beam_width]

    return select_fn


def run_routing_phase(
    repo_path: Path,
    query_file: Path,
    select_fn: SelectFn,
    repo_name: str = "local",
    results_path: Path | None = None,
) -> list[MetricRecord]:
    """Run routing phase: sweep control grid, collect metrics per query per setting.

    If results_path is provided, writes results incrementally after each test.
    """
    from bench.harness import append_results

    queries = load_queries(query_file)
    records: list[MetricRecord] = []

    for query in queries:
        relevant_map = {r["path"]: r["relevance"] for r in query.relevant}

        for control in SRT_CONTROL_GRID:
            now = datetime.now(timezone.utc).isoformat(timespec="seconds")
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

            batch: list[MetricRecord] = []
            for metric, value in [
                ("ndcg@10", ndcg),
                ("cost_usd", cost),
                ("latency_s", result.elapsed_s),
                ("tokens_loaded", result.tokens_loaded),
                ("llm_calls", result.llm_calls),
            ]:
                batch.append(MetricRecord(
                    now, "routing", repo_name, "srt",
                    query.id, control_json, metric, value,
                ))

            records.extend(batch)
            if results_path:
                append_results(results_path, batch)

    return records


def run_dilution_ablation(
    repo_path: Path,
    query_file: Path,
    select_fn: SelectFn,
    repo_name: str = "local",
    results_path: Path | None = None,
) -> list[MetricRecord]:
    """Run dilution ablation: shared descent traces, three penalty conditions.

    Conditions:
    - srt/no_penalty: baseline (mu=0)
    - srt/log_dilution: log(1 + n_l) penalty
    - srt/ratio_dilution: rho_l penalty
    """
    from bench.harness import append_results

    queries = load_queries(query_file)
    records: list[MetricRecord] = []

    for query in queries:
        relevant_map = {r["path"]: r["relevance"] for r in query.relevant}
        relevant_set = set(relevant_map.keys())

        for control in SRT_CONTROL_GRID:
            now = datetime.now(timezone.utc).isoformat(timespec="seconds")
            control_json = json.dumps(control, sort_keys=True)

            # Single descent trace shared across all conditions
            result = simulate_descent(
                repo_path=repo_path,
                question=query.question,
                select_fn=select_fn,
                beam_width=control["beam_width"],
                max_depth=control["max_depth"],
                token_budget=control["token_budget"],
            )

            # Enrich telemetry with ground truth
            enrich_telemetry(result.level_telemetry, relevant_set)

            # Compute shared metrics
            ndcg = ndcg_at_k(result.files_reached, relevant_map, k=10)
            prec = precision(result.files_reached, relevant_set)
            rec = recall(result.files_reached, relevant_set)
            mrr_val = mrr(result.files_reached, relevant_set)
            log_d = log_dilution_penalty(result.level_telemetry)
            ratio_d = ratio_dilution_penalty(result.level_telemetry)

            # Telemetry aggregates
            n_cand_mean = (
                sum(t.n_candidates for t in result.level_telemetry) / len(result.level_telemetry)
                if result.level_telemetry else 0.0
            )
            rho_mean = (
                sum(t.rho_l for t in result.level_telemetry) / len(result.level_telemetry)
                if result.level_telemetry else 0.0
            )

            # Emit metrics for each condition
            conditions = {
                "srt/no_penalty": [
                    ("ndcg@10", ndcg), ("precision", prec), ("recall", rec), ("mrr", mrr_val),
                    ("n_candidates_mean", n_cand_mean), ("rho_mean", rho_mean),
                ],
                "srt/log_dilution": [
                    ("ndcg@10", ndcg), ("precision", prec), ("recall", rec), ("mrr", mrr_val),
                    ("log_dilution_D", log_d),
                    ("n_candidates_mean", n_cand_mean), ("rho_mean", rho_mean),
                ],
                "srt/ratio_dilution": [
                    ("ndcg@10", ndcg), ("precision", prec), ("recall", rec), ("mrr", mrr_val),
                    ("ratio_dilution_D", ratio_d),
                    ("n_candidates_mean", n_cand_mean), ("rho_mean", rho_mean),
                ],
            }

            for system, metrics in conditions.items():
                batch: list[MetricRecord] = []
                for metric, value in metrics:
                    batch.append(MetricRecord(
                        now, "dilution", repo_name, system,
                        query.id, control_json, metric, value,
                    ))
                records.extend(batch)
                if results_path:
                    append_results(results_path, batch)

    return records


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="SRT routing benchmark")
    parser.add_argument("--repo", type=Path, required=True, help="Path to repo with .sem/ records")
    parser.add_argument("--queries", type=Path, required=True, help="Query YAML file")
    parser.add_argument("--results", type=Path, default=Path("results.tsv"), help="Output TSV")
    parser.add_argument("--repo-name", default="local")
    parser.add_argument("--dilution", action="store_true", help="Run dilution ablation")
    parser.add_argument("--model", default="BAAI/bge-small-en-v1.5")
    args = parser.parse_args()

    select_fn = make_embedding_select_fn(args.repo, model=args.model)
    if args.dilution:
        print("Running dilution ablation...")
        records = run_dilution_ablation(
            args.repo, args.queries, select_fn,
            repo_name=args.repo_name, results_path=args.results,
        )
        print(f"Done: {len(records)} metric records written to {args.results}")
    else:
        print("Running routing phase...")
        records = run_routing_phase(
            args.repo, args.queries, select_fn,
            repo_name=args.repo_name, results_path=args.results,
        )
        print(f"Done: {len(records)} metric records written to {args.results}")

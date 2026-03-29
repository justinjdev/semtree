#!/usr/bin/env python3
"""
Compute hypervolume of the dominated region in (latency, quality) space.

For embedding-only systems (no LLM at query time), cost = 0, so the 3D
hypervolume reduces to 2D: area under the Pareto staircase in utility space.

Normalization:
    u_l = 1 - (latency / max_latency)   [higher is better]
    u_a = ndcg                            [already in [0,1]]
    Reference point: (0, 0)

Usage:
    python3 bench/hypervolume.py bench/results/turborepo-40q-full.tsv \
        --queries bench/queries/turborepo.yaml

    # Append hypervolume rows to a file:
    python3 bench/hypervolume.py bench/results/turborepo-40q-full.tsv \
        --queries bench/queries/turborepo.yaml \
        --add-to bench/results/hypervolume-report.tsv
"""

import argparse
import json
import random
import sys
from collections import defaultdict
from pathlib import Path

import yaml


def load_tsv(path):
    """Load TSV into list of dicts."""
    rows = []
    with open(path) as f:
        header = next(f).strip().split("\t")
        for line in f:
            parts = line.strip().split("\t")
            if len(parts) != len(header):
                continue
            rows.append(dict(zip(header, parts)))
    return rows


def build_operating_points(rows):
    """
    Group rows into per-system, per-query operating points.

    Returns:
        points[system][query_id] = [(latency, ndcg), ...]
        One operating point per control setting.
    """
    # Intermediate: keyed by (system, query_id, control_json) -> {metric: value}
    raw = defaultdict(dict)
    for row in rows:
        key = (row["system"], row["query_id"], row["control_json"])
        raw[key][row["metric"]] = float(row["value"])

    points = defaultdict(lambda: defaultdict(list))
    for (system, qid, _ctrl), metrics in raw.items():
        lat = metrics.get("latency_s", 0.0)
        ndcg = metrics.get("ndcg@10", 0.0)
        points[system][qid].append((lat, ndcg))

    return points


def pareto_frontier_2d(pts):
    """
    Compute 2D Pareto frontier in utility space (u_l, u_a), both maximized.

    Input: list of (u_l, u_a) tuples.
    Returns: sorted Pareto-optimal points [(u_l, u_a), ...] sorted by u_l ascending.
    """
    if not pts:
        return []
    # Sort by u_l descending, then u_a descending for tie-breaking
    sorted_pts = sorted(pts, key=lambda p: (-p[0], -p[1]))
    frontier = []
    best_ua = -1.0
    for ul, ua in sorted_pts:
        if ua > best_ua:
            frontier.append((ul, ua))
            best_ua = ua
    frontier.sort(key=lambda p: p[0])
    return frontier


def hypervolume_2d(frontier):
    """
    Compute hypervolume (area) dominated by the Pareto staircase.

    Reference point: (0, 0). Frontier points are in utility space [0,1]x[0,1].
    The staircase goes right then up: for consecutive points, the rectangle
    from (prev_ul, 0) to (curr_ul, prev_ua) is counted.

    Returns: area in [0, 1].
    """
    if not frontier:
        return 0.0
    area = 0.0
    prev_ul = 0.0
    for ul, ua in frontier:
        # Rectangle from prev_ul to ul at height ua
        area += (ul - prev_ul) * ua
        prev_ul = ul
    # Final rectangle from last point to u_l = 1
    area += (1.0 - prev_ul) * frontier[-1][1]
    return area


def compute_query_hv(ops, max_latency):
    """
    Compute hypervolume for a single query's operating points.

    Args:
        ops: list of (latency_s, ndcg) raw operating points
        max_latency: normalization ceiling for latency

    Returns: hypervolume in [0, 1]
    """
    if not ops or max_latency <= 0:
        return 0.0
    # Normalize to utility space
    util_pts = []
    for lat, ndcg in ops:
        u_l = max(0.0, 1.0 - (lat / max_latency))
        u_a = max(0.0, min(1.0, ndcg))
        util_pts.append((u_l, u_a))
    frontier = pareto_frontier_2d(util_pts)
    return hypervolume_2d(frontier)


def bootstrap_ci(values, n_resamples=1000, ci=0.95, seed=42):
    """
    Bootstrap confidence interval over a list of values.

    Returns: (mean, lo, hi)
    """
    if not values:
        return 0.0, 0.0, 0.0
    rng = random.Random(seed)
    n = len(values)
    means = []
    for _ in range(n_resamples):
        sample = [values[rng.randint(0, n - 1)] for _ in range(n)]
        means.append(sum(sample) / len(sample))
    means.sort()
    alpha = (1 - ci) / 2
    lo = means[int(alpha * n_resamples)]
    hi = means[int((1 - alpha) * n_resamples)]
    mean = sum(values) / n
    return mean, lo, hi


def main():
    parser = argparse.ArgumentParser(
        description="Compute hypervolume of dominated region in (latency, quality) space"
    )
    parser.add_argument("tsv", type=Path, help="Results TSV from run_benchmark.py")
    parser.add_argument("--queries", type=Path, default=None,
                        help="Query YAML (for category breakdown)")
    parser.add_argument("--add-to", type=Path, default=None,
                        help="Append hypervolume rows to this TSV file")
    parser.add_argument("--seed", type=int, default=42,
                        help="Bootstrap RNG seed (default: 42)")
    args = parser.parse_args()

    rows = load_tsv(args.tsv)
    if not rows:
        print("No data in TSV", file=sys.stderr)
        sys.exit(1)

    points = build_operating_points(rows)
    systems = sorted(points.keys())

    # Global max latency across all systems and queries for normalization
    max_latency = 0.0
    for system in systems:
        for qid, ops in points[system].items():
            for lat, _ndcg in ops:
                max_latency = max(max_latency, lat)

    if max_latency <= 0:
        print("All latencies are 0, cannot normalize", file=sys.stderr)
        sys.exit(1)

    # Load categories if provided
    cat_map = {}  # query_id -> category
    categories = []
    if args.queries:
        with open(args.queries) as f:
            qs = yaml.safe_load(f)["queries"]
        for q in qs:
            cat_map[q["id"]] = q.get("category", "unknown")
        categories = sorted(set(cat_map.values()))

    # Compute per-system, per-query hypervolume
    sys_hvs = {}  # system -> [hv_per_query]
    sys_cat_hvs = {}  # system -> {category -> [hv_per_query]}
    all_qids = sorted(set(qid for s in systems for qid in points[s]))

    for system in systems:
        hvs = []
        cat_hvs = defaultdict(list)
        for qid in all_qids:
            ops = points[system].get(qid, [])
            hv = compute_query_hv(ops, max_latency)
            hvs.append(hv)
            if qid in cat_map:
                cat_hvs[cat_map[qid]].append(hv)
        sys_hvs[system] = hvs
        sys_cat_hvs[system] = dict(cat_hvs)

    # Print report
    cat_headers = categories if categories else []
    cat_col_width = max(10, *(len(c) for c in cat_headers)) if cat_headers else 0

    header = f"{'System':<15s} {'HV':>6s} {'95% CI':>16s}"
    for cat in cat_headers:
        header += f"  {cat:>{cat_col_width}s}"
    print(header)
    print("-" * len(header))

    report_rows = []
    for system in systems:
        mean, lo, hi = bootstrap_ci(sys_hvs[system], seed=args.seed)
        line = f"{system:<15s} {mean:>6.3f} [{lo:>6.3f}, {hi:>6.3f}]"
        cat_means = {}
        for cat in cat_headers:
            cat_vals = sys_cat_hvs[system].get(cat, [])
            cm = sum(cat_vals) / len(cat_vals) if cat_vals else 0.0
            cat_means[cat] = cm
            line += f"  {cm:>{cat_col_width}.3f}"
        print(line)
        report_rows.append((system, mean, lo, hi, cat_means))

    print(f"\nmax_latency={max_latency:.4f}s  queries={len(all_qids)}  "
          f"systems={len(systems)}")

    # --add-to: append to TSV
    if args.add_to:
        write_header = not args.add_to.exists()
        args.add_to.parent.mkdir(parents=True, exist_ok=True)
        with open(args.add_to, "a") as f:
            if write_header:
                cols = ["system", "hv_mean", "hv_ci_lo", "hv_ci_hi"]
                for cat in cat_headers:
                    cols.append(f"hv_{cat}")
                f.write("\t".join(cols) + "\n")
            for system, mean, lo, hi, cat_means in report_rows:
                vals = [system, f"{mean:.4f}", f"{lo:.4f}", f"{hi:.4f}"]
                for cat in cat_headers:
                    vals.append(f"{cat_means.get(cat, 0.0):.4f}")
                f.write("\t".join(vals) + "\n")
        print(f"\nAppended to {args.add_to}", file=sys.stderr)


if __name__ == "__main__":
    main()

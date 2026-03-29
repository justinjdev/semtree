#!/usr/bin/env python3
"""
Analyze parameter sweep results from run_benchmark.py TSV output.

Usage:
    python3 bench/analyze_sweep.py bench/results/<repo>-sweep.tsv

Produces:
    - Per-config aggregate NDCG and latency
    - Best configs per query category
    - Pareto frontier (NDCG vs latency)
    - Uniform vs Waterfill comparison
"""

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path


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


def parse_control(ctrl_json):
    """Parse control JSON, return hashable tuple."""
    d = json.loads(ctrl_json)
    return tuple(sorted(d.items()))


def analyze(rows, system_prefix="srt"):
    """Analyze SRT sweep results."""
    # Group by (system, control_json, query_id) → metrics
    configs = defaultdict(lambda: defaultdict(float))
    latencies = defaultdict(lambda: defaultdict(list))

    for row in rows:
        system = row["system"]
        if not system.startswith(system_prefix):
            continue
        ctrl = row["control_json"]
        qid = row["query_id"]
        metric = row["metric"]
        value = float(row["value"])

        key = (system, ctrl)
        if metric == "ndcg@10":
            configs[key][qid] = max(configs[key][qid], value)
        elif metric == "latency_s":
            latencies[key][qid].append(value)

    if not configs:
        print(f"No rows matching system prefix '{system_prefix}'", file=sys.stderr)
        return

    # Aggregate per config
    results = []
    for (system, ctrl_json), qid_scores in configs.items():
        ctrl = json.loads(ctrl_json)
        avg_ndcg = sum(qid_scores.values()) / len(qid_scores) if qid_scores else 0
        hits = sum(1 for v in qid_scores.values() if v > 0)
        total = len(qid_scores)

        all_lats = [l for lats in latencies[(system, ctrl_json)].values() for l in lats]
        all_lats.sort()
        p50_lat = all_lats[len(all_lats) // 2] if all_lats else 0

        results.append({
            "system": system,
            "beam_width": ctrl.get("beam_width", "?"),
            "max_depth": ctrl.get("max_depth", "?"),
            "beam_policy": ctrl.get("beam_policy", "uniform"),
            "avg_ndcg": avg_ndcg,
            "hits": hits,
            "total": total,
            "p50_ms": p50_lat * 1000,
            "per_query": dict(qid_scores),
        })

    results.sort(key=lambda r: r["avg_ndcg"], reverse=True)

    # --- Top configs table ---
    print("=" * 90)
    print("TOP CONFIGS BY NDCG")
    print("=" * 90)
    print(f"{'Rank':>4}  {'System':<10} {'BW':>3} {'MD':>3} {'Policy':<10} {'NDCG':>7} {'Hits':>7} {'P50ms':>8}")
    print("-" * 90)
    for i, r in enumerate(results[:20]):
        print(f"{i+1:>4}  {r['system']:<10} {r['beam_width']:>3} {r['max_depth']:>3} "
              f"{r['beam_policy']:<10} {r['avg_ndcg']:>7.3f} {r['hits']:>3}/{r['total']:<3} {r['p50_ms']:>8.1f}")

    # --- Pareto frontier (NDCG vs latency) ---
    print(f"\n{'=' * 90}")
    print("PARETO FRONTIER (highest NDCG for each latency tier)")
    print("=" * 90)
    pareto = []
    results_by_lat = sorted(results, key=lambda r: r["p50_ms"])
    best_ndcg = -1
    for r in results_by_lat:
        if r["avg_ndcg"] > best_ndcg:
            best_ndcg = r["avg_ndcg"]
            pareto.append(r)

    print(f"{'System':<10} {'BW':>3} {'MD':>3} {'Policy':<10} {'NDCG':>7} {'Hits':>7} {'P50ms':>8}")
    print("-" * 90)
    for r in pareto:
        print(f"{r['system']:<10} {r['beam_width']:>3} {r['max_depth']:>3} "
              f"{r['beam_policy']:<10} {r['avg_ndcg']:>7.3f} {r['hits']:>3}/{r['total']:<3} {r['p50_ms']:>8.1f}")

    # --- Uniform vs Waterfill ---
    print(f"\n{'=' * 90}")
    print("UNIFORM vs WATERFILL (matched configs)")
    print("=" * 90)
    by_config = defaultdict(dict)
    for r in results:
        key = (r["system"], r["beam_width"], r["max_depth"])
        by_config[key][r["beam_policy"]] = r

    print(f"{'System':<10} {'BW':>3} {'MD':>3}  {'Uni NDCG':>9} {'WF NDCG':>9} {'Delta':>7}  {'Uni P50':>8} {'WF P50':>8}")
    print("-" * 90)
    for key in sorted(by_config):
        u = by_config[key].get("uniform")
        w = by_config[key].get("waterfill")
        if not u or not w:
            continue
        delta = w["avg_ndcg"] - u["avg_ndcg"]
        sys_name, bw, md = key
        print(f"{sys_name:<10} {bw:>3} {md:>3}  {u['avg_ndcg']:>9.3f} {w['avg_ndcg']:>9.3f} {delta:>+7.3f}  "
              f"{u['p50_ms']:>7.1f}ms {w['p50_ms']:>7.1f}ms")

    # --- Per beam_width aggregate ---
    print(f"\n{'=' * 90}")
    print("AGGREGATE BY BEAM WIDTH")
    print("=" * 90)
    by_bw = defaultdict(list)
    for r in results:
        by_bw[r["beam_width"]].append(r)
    print(f"{'BW':>3}  {'Avg NDCG':>9} {'Best NDCG':>10} {'Avg Hits':>9} {'Avg P50ms':>10}")
    print("-" * 50)
    for bw in sorted(by_bw):
        group = by_bw[bw]
        avg_n = sum(r["avg_ndcg"] for r in group) / len(group)
        best_n = max(r["avg_ndcg"] for r in group)
        avg_h = sum(r["hits"] for r in group) / len(group)
        avg_l = sum(r["p50_ms"] for r in group) / len(group)
        print(f"{bw:>3}  {avg_n:>9.3f} {best_n:>10.3f} {avg_h:>9.1f} {avg_l:>10.1f}")

    # --- Per max_depth aggregate ---
    print(f"\n{'=' * 90}")
    print("AGGREGATE BY MAX DEPTH")
    print("=" * 90)
    by_md = defaultdict(list)
    for r in results:
        by_md[r["max_depth"]].append(r)
    print(f"{'MD':>3}  {'Avg NDCG':>9} {'Best NDCG':>10} {'Avg Hits':>9} {'Avg P50ms':>10}")
    print("-" * 50)
    for md in sorted(by_md):
        group = by_md[md]
        avg_n = sum(r["avg_ndcg"] for r in group) / len(group)
        best_n = max(r["avg_ndcg"] for r in group)
        avg_h = sum(r["hits"] for r in group) / len(group)
        avg_l = sum(r["p50_ms"] for r in group) / len(group)
        print(f"{md:>3}  {avg_n:>9.3f} {best_n:>10.3f} {avg_h:>9.1f} {avg_l:>10.1f}")


def main():
    parser = argparse.ArgumentParser(description="Analyze parameter sweep results")
    parser.add_argument("tsv", type=Path, help="Results TSV from run_benchmark.py")
    parser.add_argument("--system", default="srt", help="System prefix to analyze (default: srt)")
    args = parser.parse_args()

    rows = load_tsv(args.tsv)
    analyze(rows, system_prefix=args.system)


if __name__ == "__main__":
    main()

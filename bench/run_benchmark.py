#!/usr/bin/env python3
"""
Run full multi-system benchmark: SRT (Rust), grep, ripgrep, Shire.

Usage:
    python3 bench/run_benchmark.py <repo_path> <query_yaml> [--results results.tsv]

Systems:
    srt-cold     Rust CLI, fresh process per query (no daemon)
    srt-warm     Rust CLI via Unix socket daemon
    grep         Keyword extraction + grep
    ripgrep      Keyword extraction + rg
    shire        FTS5 + vector RAG via MCP (if shire binary available)

Output:
    TSV file with per-query-per-system-per-setting metrics (NDCG@10, latency, etc.)
    Summary table to stderr
"""

import argparse
import json
import math
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import time
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

import yaml

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

SEMTREE = Path(__file__).parent.parent / "cli" / "target" / "release" / "semtree"
SOCKET_PATH = "/tmp/semtree-bench.sock"

SRT_GRID = [
    {"beam_width": bw, "max_depth": md, "beam_policy": bp}
    for bw in [2, 3, 5, 7, 10]
    for md in [3, 5, 7, 10]
    for bp in ["uniform", "waterfill"]
]

GREP_GRID = [{"max_files": mf} for mf in [5, 10, 20]]
RG_GRID = [{"max_files": mf} for mf in [5, 10, 20]]
SHIRE_GRID = [{"strategy": s, "limit": lim} for s in ["symbols", "files", "combined"] for lim in [10, 20]]

STOP_WORDS = {"how", "does", "the", "a", "an", "is", "what", "where", "which", "when",
              "do", "are", "in", "of", "to", "for", "and", "or", "with", "that", "its",
              "from", "by", "into", "this"}

# ---------------------------------------------------------------------------
# NDCG
# ---------------------------------------------------------------------------

def ndcg_at_k(retrieved, relevant_map, k=10):
    if not relevant_map:
        return 0.0
    dcg = sum((2 ** relevant_map.get(p, 0) - 1) / math.log2(i + 2) for i, p in enumerate(retrieved[:k]))
    ideal = sorted(relevant_map.values(), reverse=True)[:k]
    idcg = sum((2 ** r - 1) / math.log2(i + 2) for i, r in enumerate(ideal))
    return dcg / idcg if idcg > 0 else 0.0

# ---------------------------------------------------------------------------
# TSV output
# ---------------------------------------------------------------------------

def append_tsv(path, rows):
    write_header = not path.exists()
    with open(path, "a") as f:
        if write_header:
            f.write("timestamp\tphase\trepo\tsystem\tquery_id\tcontrol_json\tmetric\tvalue\n")
        for row in rows:
            f.write("\t".join(str(v) for v in row) + "\n")

def write_metrics(results_path, repo_name, system, query_id, control, metrics):
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    ctrl_json = json.dumps(control, sort_keys=True)
    rows = [(now, "routing", repo_name, system, query_id, ctrl_json, m, v) for m, v in metrics]
    append_tsv(results_path, rows)

# ---------------------------------------------------------------------------
# SRT routing (Rust CLI)
# ---------------------------------------------------------------------------

def parse_route_output(stdout):
    """Parse semtree route stdout → score-ranked file candidates."""
    dirs_descended = set()
    all_candidates = []
    for line in stdout.split("\n"):
        stripped = line.strip()
        if stripped.startswith("---") and "children" in stripped:
            dir_name = stripped.split("---")[1].strip().split("(")[0].strip()
            dirs_descended.add(dir_name)
        elif stripped and stripped[0].isdigit():
            parts = stripped.split(None, 2)
            if len(parts) >= 2:
                try:
                    all_candidates.append((parts[1], float(parts[0])))
                except ValueError:
                    pass
    files = [(p, s) for p, s in all_candidates
             if p not in dirs_descended and not any(d.startswith(p + "/") for d in dirs_descended)]
    files.sort(key=lambda x: x[1], reverse=True)
    return [p for p, _ in files]

def run_srt_cold(repo, queries, repo_name, results_path):
    print("  SRT cold...", file=sys.stderr, end=" ", flush=True)
    for q in queries:
        rel = {r["path"]: r["relevance"] for r in q.get("relevant", [])}
        for ctrl in SRT_GRID:
            t0 = time.monotonic()
            cmd = [str(SEMTREE), "route", q["question"], str(repo),
                   "--beam-width", str(ctrl["beam_width"]),
                   "--max-depth", str(ctrl["max_depth"]),
                   "--beam-policy", ctrl.get("beam_policy", "uniform")]
            proc = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
            latency = time.monotonic() - t0
            files = parse_route_output(proc.stdout)
            ndcg = ndcg_at_k(files, rel)
            write_metrics(results_path, repo_name, "srt-cold", q["id"], ctrl,
                          [("ndcg@10", ndcg), ("latency_s", latency)])
    print("done", file=sys.stderr)

def run_srt_warm(repo, queries, repo_name, results_path):
    print("  SRT warm (daemon)...", file=sys.stderr, end=" ", flush=True)
    daemon = subprocess.Popen([str(SEMTREE), "serve", "--socket", SOCKET_PATH],
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    time.sleep(3)
    try:
        # Warmup
        _daemon_call({"method": "route", "params": {"query": "warmup", "path": str(repo),
                       "beam_width": 1, "max_depth": 1}})
        for q in queries:
            rel = {r["path"]: r["relevance"] for r in q.get("relevant", [])}
            for ctrl in SRT_GRID:
                t0 = time.monotonic()
                resp = _daemon_call({"method": "route", "params": {
                    "query": q["question"], "path": str(repo),
                    "beam_width": ctrl["beam_width"], "max_depth": ctrl["max_depth"],
                    "beam_policy": ctrl.get("beam_policy", "uniform")}})
                latency = time.monotonic() - t0
                files = _parse_daemon_route(resp)
                ndcg = ndcg_at_k(files, rel)
                write_metrics(results_path, repo_name, "srt-warm", q["id"], ctrl,
                              [("ndcg@10", ndcg), ("latency_s", latency)])
    finally:
        daemon.terminate()
        daemon.wait(timeout=5)
        if os.path.exists(SOCKET_PATH):
            os.unlink(SOCKET_PATH)
    print("done", file=sys.stderr)

def _daemon_call(request):
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(SOCKET_PATH)
    sock.sendall((json.dumps(request) + "\n").encode())
    resp = b""
    while b"\n" not in resp:
        chunk = sock.recv(65536)
        if not chunk:
            break
        resp += chunk
    sock.close()
    return json.loads(resp.decode().strip())

def _parse_daemon_route(resp):
    levels = resp.get("result", {}).get("levels", [])
    dir_set = {l["dir"] for l in levels}
    candidates = []
    for level in levels:
        for sel in level.get("selected", []):
            p, s = sel["path"], sel["score"]
            if p not in dir_set and not any(d.startswith(p + "/") for d in dir_set):
                candidates.append((p, s))
    candidates.sort(key=lambda x: x[1], reverse=True)
    return [p for p, _ in candidates]

# ---------------------------------------------------------------------------
# Grep / ripgrep
# ---------------------------------------------------------------------------

def extract_keywords(question):
    words = re.findall(r"[a-zA-Z_]+", question.lower())
    return [w for w in words if w not in STOP_WORDS and len(w) > 2]

def run_grep(repo, queries, repo_name, results_path):
    print("  grep...", file=sys.stderr, end=" ", flush=True)
    for q in queries:
        rel = {r["path"]: r["relevance"] for r in q.get("relevant", [])}
        keywords = extract_keywords(q["question"])
        for ctrl in GREP_GRID:
            t0 = time.monotonic()
            found = defaultdict(int)
            for kw in keywords:
                proc = subprocess.run(
                    ["grep", "-rl", "--include=*.rs", "--include=*.go", "--include=*.ts",
                     "--include=*.js", "--include=*.py", "--include=*.md",
                     "--exclude-dir=node_modules", "--exclude-dir=.git",
                     "--exclude-dir=.sem", "--exclude-dir=target", kw, "."],
                    cwd=repo, capture_output=True, text=True)
                if proc.returncode == 0:
                    for line in proc.stdout.strip().split("\n"):
                        p = line.lstrip("./")
                        if p and not p.startswith(".sem/"):
                            found[p] += 1
            ranked = sorted(found, key=lambda p: found[p], reverse=True)[:ctrl["max_files"]]
            latency = time.monotonic() - t0
            ndcg = ndcg_at_k(ranked, rel)
            write_metrics(results_path, repo_name, "grep", q["id"], ctrl,
                          [("ndcg@10", ndcg), ("latency_s", latency)])
    print("done", file=sys.stderr)

def run_rg(repo, queries, repo_name, results_path):
    if not shutil.which("rg"):
        print("  ripgrep: not installed, skipping", file=sys.stderr)
        return
    print("  ripgrep...", file=sys.stderr, end=" ", flush=True)
    for q in queries:
        rel = {r["path"]: r["relevance"] for r in q.get("relevant", [])}
        keywords = extract_keywords(q["question"])
        for ctrl in RG_GRID:
            t0 = time.monotonic()
            found = defaultdict(int)
            for kw in keywords:
                proc = subprocess.run(
                    ["rg", "-l", "--no-heading", "-t", "rust", "-t", "go", "-t", "ts",
                     "-t", "js", "-t", "py", "-t", "md", "-g", "!.sem/", "-g", "!target/", kw],
                    cwd=repo, capture_output=True, text=True)
                if proc.returncode == 0:
                    for line in proc.stdout.strip().split("\n"):
                        if line.strip():
                            found[line.strip()] += 1
            ranked = sorted(found, key=lambda p: found[p], reverse=True)[:ctrl["max_files"]]
            latency = time.monotonic() - t0
            ndcg = ndcg_at_k(ranked, rel)
            write_metrics(results_path, repo_name, "ripgrep", q["id"], ctrl,
                          [("ndcg@10", ndcg), ("latency_s", latency)])
    print("done", file=sys.stderr)

# ---------------------------------------------------------------------------
# Shire
# ---------------------------------------------------------------------------

def run_shire(repo, queries, repo_name, results_path):
    if not shutil.which("shire"):
        print("  shire: not installed, skipping", file=sys.stderr)
        return
    # Check if shire index exists
    if not (repo / ".shire" / "index.db").exists():
        print("  shire: no index at repo, skipping", file=sys.stderr)
        return
    print("  shire...", file=sys.stderr, end=" ", flush=True)
    proc = subprocess.Popen(["shire", "serve", "--root", str(repo)],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    # Init MCP
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "initialize",
                                  "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                                             "clientInfo": {"name": "bench", "version": "0.1"}}, "id": 1}) + "\n")
    proc.stdin.flush()
    proc.stdout.readline()
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    proc.stdin.flush()

    msg_id = 10
    try:
        for q in queries:
            rel = {r["path"]: r["relevance"] for r in q.get("relevant", [])}
            for ctrl in SHIRE_GRID:
                t0 = time.monotonic()
                file_paths = []
                if ctrl["strategy"] in ("symbols", "combined"):
                    msg_id += 1
                    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "tools/call",
                        "params": {"name": "search_symbols",
                                   "arguments": {"query": q["question"], "limit": ctrl["limit"]}},
                        "id": msg_id}) + "\n")
                    proc.stdin.flush()
                    resp = json.loads(proc.stdout.readline())
                    for sym in json.loads(resp.get("result", {}).get("content", [{}])[0].get("text", "[]")):
                        fp = sym.get("file_path", "")
                        if fp and fp not in file_paths:
                            file_paths.append(fp)
                if ctrl["strategy"] in ("files", "combined"):
                    msg_id += 1
                    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "tools/call",
                        "params": {"name": "search_files",
                                   "arguments": {"query": q["question"], "limit": ctrl["limit"]}},
                        "id": msg_id}) + "\n")
                    proc.stdin.flush()
                    resp = json.loads(proc.stdout.readline())
                    for f in json.loads(resp.get("result", {}).get("content", [{}])[0].get("text", "[]")):
                        fp = f.get("path", "")
                        if fp and fp not in file_paths:
                            file_paths.append(fp)
                latency = time.monotonic() - t0
                ndcg = ndcg_at_k(file_paths, rel)
                write_metrics(results_path, repo_name, "shire", q["id"], ctrl,
                              [("ndcg@10", ndcg), ("latency_s", latency)])
    finally:
        proc.terminate()
    print("done", file=sys.stderr)

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

def print_summary(results_path, queries):
    if not results_path.exists():
        return
    records = defaultdict(lambda: defaultdict(float))
    latencies = defaultdict(list)
    with open(results_path) as f:
        next(f)  # header
        for line in f:
            parts = line.strip().split("\t")
            if len(parts) != 8:
                continue
            _, _, _, system, qid, _, metric, value = parts
            value = float(value)
            if metric == "ndcg@10":
                records[system][qid] = max(records[system][qid], value)
            if metric == "latency_s":
                latencies[system].append(value)

    print("\n" + "=" * 70, file=sys.stderr)
    print(f"{'System':<15s} {'Best NDCG':>10s} {'Hits':>8s} {'P50 lat':>10s}", file=sys.stderr)
    print("-" * 70, file=sys.stderr)
    for sys_name in sorted(records):
        best = records[sys_name]
        mean_ndcg = sum(best.values()) / len(best) if best else 0
        hits = sum(1 for v in best.values() if v > 0)
        total = len(queries)
        lat = sorted(latencies.get(sys_name, [0]))
        p50 = lat[len(lat) // 2] if lat else 0
        print(f"{sys_name:<15s} {mean_ndcg:>10.3f} {hits:>5d}/{total:<2d}  {p50*1000:>8.1f}ms", file=sys.stderr)

    print("\nPer-query best NDCG:", file=sys.stderr)
    systems = sorted(records)
    header = f"{'query':>6s}"
    for s in systems:
        header += f"  {s:>12s}"
    print(header, file=sys.stderr)
    for q in queries:
        row = f"{q['id']:>6s}"
        for s in systems:
            v = records[s].get(q["id"], 0)
            row += f"  {v:>12.3f}"
        print(row, file=sys.stderr)

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Run multi-system SRT benchmark")
    parser.add_argument("repo", type=Path, help="Repository path")
    parser.add_argument("queries", type=Path, help="Query YAML file")
    parser.add_argument("--results", type=Path, default=None, help="Results TSV (default: bench/results/<repo>-benchmark.tsv)")
    parser.add_argument("--systems", default="srt-cold,srt-warm,grep,ripgrep,shire", help="Comma-separated systems to run")
    parser.add_argument("--repo-name", default=None, help="Repo name for results (default: dirname)")
    args = parser.parse_args()

    repo = args.repo.resolve()
    repo_name = args.repo_name or repo.name
    results_path = args.results or (Path(__file__).parent / "results" / f"{repo_name}-benchmark.tsv")
    results_path.parent.mkdir(parents=True, exist_ok=True)

    with open(args.queries) as f:
        queries = yaml.safe_load(f)["queries"]

    systems = args.systems.split(",")
    print(f"Benchmark: {repo_name} ({len(queries)} queries, systems: {', '.join(systems)})", file=sys.stderr)
    print(f"Results: {results_path}", file=sys.stderr)

    if "srt-cold" in systems:
        run_srt_cold(repo, queries, repo_name, results_path)
    if "srt-warm" in systems:
        run_srt_warm(repo, queries, repo_name, results_path)
    if "grep" in systems:
        run_grep(repo, queries, repo_name, results_path)
    if "ripgrep" in systems:
        run_rg(repo, queries, repo_name, results_path)
    if "shire" in systems:
        run_shire(repo, queries, repo_name, results_path)

    print_summary(results_path, queries)

if __name__ == "__main__":
    main()

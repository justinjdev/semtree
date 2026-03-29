#!/bin/bash
# Run semtree route benchmark against a query set and compute NDCG.
# Usage: ./bench/run_routing.sh <repo_path> <query_yaml>

set -euo pipefail

SEMTREE="./cli/target/release/semtree"
REPO="${1:?Usage: $0 <repo_path> <query_yaml>}"
QUERY_FILE="${2:?Usage: $0 <repo_path> <query_yaml>}"
BEAM="${3:-5}"
MAX_DEPTH="${4:-10}"

# Parse queries from YAML (lightweight — just extract id, category, question, relevant paths+scores)
python3 -c "
import yaml, json, math, sys, subprocess, time

with open('$QUERY_FILE') as f:
    data = yaml.safe_load(f)

queries = data['queries']

def ndcg_at_k(retrieved, relevant_map, k=10):
    if not relevant_map:
        return 0.0
    dcg = 0.0
    for i, path in enumerate(retrieved[:k]):
        rel = relevant_map.get(path, 0)
        dcg += (2**rel - 1) / math.log2(i + 2)
    ideal_rels = sorted(relevant_map.values(), reverse=True)[:k]
    idcg = sum((2**r - 1) / math.log2(i + 2) for i, r in enumerate(ideal_rels))
    if idcg == 0:
        return 0.0
    return dcg / idcg

results_by_cat = {}
all_ndcg = []
all_hits = 0
total_ms = 0

for q in queries:
    qid = q['id']
    question = q['question']
    category = q['category']
    relevant_map = {r['path']: r['relevance'] for r in q.get('relevant', [])}

    # Run semtree route
    t0 = time.monotonic()
    proc = subprocess.run(
        ['$SEMTREE', 'route', question, '$REPO', '--beam-width', '$BEAM', '--max-depth', '$MAX_DEPTH'],
        capture_output=True, text=True
    )
    elapsed_ms = (time.monotonic() - t0) * 1000

    # Parse output: lines like '  0.4321  path/to/file  summary...'
    files = []
    for line in proc.stdout.strip().split('\n'):
        line = line.strip()
        if not line or line.startswith('---'):
            continue
        parts = line.split(None, 2)
        if len(parts) >= 2:
            try:
                score = float(parts[0])
                path = parts[1]
                files.append((path, score))
            except ValueError:
                continue

    # Sort by score descending, extract paths
    files.sort(key=lambda x: x[1], reverse=True)
    retrieved = [p for p, s in files]

    ndcg = ndcg_at_k(retrieved, relevant_map)
    hit = 1 if any(p in relevant_map for p in retrieved) else 0

    results_by_cat.setdefault(category, []).append(ndcg)
    all_ndcg.append(ndcg)
    all_hits += hit
    total_ms += elapsed_ms

    status = 'HIT' if hit else 'MISS'
    print(f'  {qid} [{category:13s}] NDCG={ndcg:.3f} {status:4s} ({elapsed_ms:.0f}ms) files={len(retrieved)}  {question[:60]}')

print()
print(f'=== Results (beam={\"$BEAM\"}, max_depth={\"$MAX_DEPTH\"}) ===')
print(f'  Overall NDCG:  {sum(all_ndcg)/len(all_ndcg):.3f}')
print(f'  Hits:          {all_hits}/{len(queries)}')
print(f'  Total time:    {total_ms:.0f}ms ({total_ms/len(queries):.0f}ms/query)')
print()
for cat in ['focused', 'module', 'cross-cutting']:
    scores = results_by_cat.get(cat, [])
    if scores:
        avg = sum(scores) / len(scores)
        print(f'  {cat:15s}: {avg:.3f} (n={len(scores)})')
"

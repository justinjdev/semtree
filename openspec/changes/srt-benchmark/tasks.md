## 1. Project Setup

- [ ] 1.1 Create `bench/` package directory with `__init__.py`
- [ ] 1.2 Add `semtree bench` subcommand to `cli.py` with `--phase` and `--repo` flags
- [ ] 1.3 Create `bench/repos.yaml` config with fellowship repo pinned at current commit

## 2. Benchmark Repos (bench-repos)

- [ ] 2.1 Implement `bench/repos.py`: clone repo at pinned SHA into `bench/.repos/` cache
- [ ] 2.2 Support three size tiers in `repos.yaml` (small/medium/large), start with fellowship as small
- [ ] 2.3 Add `semtree bench --clean` to remove cached repos
- [ ] 2.4 Write tests for repo clone, cache hit, and cleanup

## 3. Harness (bench-harness)

- [ ] 3.1 Implement `bench/harness.py`: phase runner that calls phase functions, collects timing, writes results
- [ ] 3.2 Implement TSV results logging (append-only to `results.tsv`)
- [ ] 3.3 Wire CLI: `semtree bench <phase>` dispatches to harness with repo selection
- [ ] 3.4 Write tests for harness timing collection and TSV output format

## 4. Build Phase (bench-build)

- [ ] 4.1 Implement `bench/build_phase.py`: run full build, measure wall-clock time, count LLM calls and nodes
- [ ] 4.2 Measure incremental no-op build time (second run with no changes)
- [ ] 4.3 Report metrics: build_time_s, llm_calls, node_count, skipped_count
- [ ] 4.4 Write test with mock summarizer verifying metric collection

## 5. Quality Phase (bench-quality)

- [ ] 5.1 Implement `bench/quality.py`: scan all `.sem/` records for structural correctness
- [ ] 5.2 Check children coverage: every child of a directory appears in its `__dir__.md` `## Children` section
- [ ] 5.3 Check frontmatter validity: required fields (path, type, content_hash) present and correct types
- [ ] 5.4 Check hash consistency: stored content_hash matches freshly computed hash
- [ ] 5.5 Check for orphan records: `.sem/` record exists but source file doesn't
- [ ] 5.6 Check deterministic rebuild: two builds produce identical hashes
- [ ] 5.7 Write tests using fixture `.sem/` records with known good/bad examples

## 6. Routing Phase (bench-routing)

- [ ] 6.1 Create `bench/queries/fellowship.yaml` with 10-15 queries and expected target files
- [ ] 6.2 Implement `bench/routing.py`: load query set, simulate descent through `.sem/__dir__.md` records
- [ ] 6.3 Implement LLM-based child selection (call claude to select relevant children from routing table)
- [ ] 6.4 Compute recall@k: fraction of expected files reached via descent
- [ ] 6.5 Report metrics: recall@5, files_opened, directories_traversed per query
- [ ] 6.6 Write tests with mock routing table and mock LLM selection

## 7. Incremental Phase (bench-incremental)

- [ ] 7.1 Implement `bench/incremental.py`: modify a known file, rebuild, verify only changed subtree re-summarized
- [ ] 7.2 Verify ancestor directories are re-summarized (hash propagation)
- [ ] 7.3 Verify unchanged files are skipped
- [ ] 7.4 Measure incremental rebuild time
- [ ] 7.5 Write tests with fixture repo and mock summarizer

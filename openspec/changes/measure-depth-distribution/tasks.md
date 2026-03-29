## 1. Query YAML Parsing in Rust

- [x] 1.1 Add `serde` and `serde_yaml` dependencies to `cli/Cargo.toml` (if not already present) for parsing query YAML files
- [x] 1.2 Define Rust structs for query YAML format: `QueryFile { queries: Vec<Query> }`, `Query { id, question, category, relevant: Vec<RelevantFile> }`, `RelevantFile { path, relevance }`
- [x] 1.3 Implement `load_queries(path: &Path) -> Result<Vec<Query>>` function in `bench.rs`
- [x] 1.4 Add unit test: parse a small inline YAML string and verify fields are extracted correctly

## 2. Depth Computation Core

- [x] 2.1 Implement `file_depth(path: &str) -> usize` that counts path components (split on `/`, count segments)
- [x] 2.2 Implement `compute_fk(queries: &[Query], weighted: bool) -> BTreeMap<usize, f64>` that builds the depth distribution from relevant files, normalized to sum to 1.0
- [x] 2.3 Implement `DepthStats` struct with fields: mean, std_dev, entropy, support_min, support_max, num_queries, num_files
- [x] 2.4 Implement `compute_stats(fk: &BTreeMap<usize, f64>) -> DepthStats` for summary statistics
- [x] 2.5 Add unit tests: verify depth computation for paths at various depths, verify f_k normalization, verify stats for known distributions

## 3. Repository Tree Structure

- [x] 3.1 Implement `repo_tree_metrics(repo_path: &Path) -> Result<(usize, BTreeMap<usize, f64>)>` that walks the directory tree and returns (max_depth, branching_factors_by_level)
- [x] 3.2 Reuse the existing `walker` module's ignore rules (skip dotfiles, dotdirs, symlinks, binary) when walking the tree
- [x] 3.3 Add unit test: create a temp directory structure and verify max depth and branching factors

## 4. CLI Integration

- [x] 4.1 Add `--queries` optional `PathBuf` argument to the `Bench` command in `main.rs`
- [x] 4.2 Add `depth-profile` phase handling in the `Bench` match arm in `main.rs`
- [x] 4.3 Wire up: when phase is `"depth-profile"` or `"all"`, call the depth profiling logic with the queries file and repo path
- [x] 4.4 Validate that `--queries` is provided when phase is `depth-profile`; exit with error if missing

## 5. TSV Output

- [x] 5.1 Emit per-query rows: `(timestamp, "depth-profile", repo, "srt", query_id, "", "depth_mean"|"depth_min"|"depth_max", value)` via `bench::append_tsv`
- [x] 5.2 Emit aggregate f_k rows: `(timestamp, "depth-profile", repo, "srt", "", "", "fk_d{N}", value)` for each depth N with nonzero mass
- [x] 5.3 Emit aggregate stats rows: `fk_mean`, `fk_std`, `fk_entropy`, `fk_support_min`, `fk_support_max`, `repo_max_depth`
- [x] 5.4 Emit branching factor rows: `B_{N}` for each level N
- [x] 5.5 Emit unweighted variants with metric names prefixed `unweighted_`

## 6. Human-Readable Summary

- [x] 6.1 Print text histogram of f_k to stderr (depth on left, bar proportional to f_k, value on right)
- [x] 6.2 Print summary line with mean, std dev, entropy, support range, and counts
- [x] 6.3 Print repo tree metrics (H, branching factors)

## 7. Per-Category Breakdown

- [x] 7.1 Group queries by `category` field and compute separate f_k distributions per category
- [x] 7.2 Emit per-category f_k rows with category name in `control_json` field
- [x] 7.3 Print per-category summary to stderr

## 8. Integration Testing

- [x] 8.1 Run `semtree bench depth-profile --queries bench/queries/glamdring.yaml --repo-path <fellowship-repo>` end-to-end and verify TSV output
- [x] 8.2 Verify the output f_k values sum to 1.0 (within floating point tolerance)
- [x] 8.3 Verify per-query depth metrics are reasonable (min <= mean <= max)

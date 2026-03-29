## Context

The SRT hyperparameter model defines f_k = Pr[relevant item at depth k]. The routing benchmark infrastructure (`bench/routing.py`) already runs queries against SRT-indexed repos with graded relevance annotations, but it only measures retrieval quality (NDCG) and cost -- it never records the depth at which relevant files live. The Rust CLI (`cli/src/bench.rs`) has a `bench` subcommand that runs quality checks but lacks routing-aware analysis.

Query files (`bench/queries/*.yaml`) already annotate relevant files with paths and graded relevance scores. The depth of each relevant file can be computed purely from its path (count path separators). No SRT descent or embedding is needed -- this is a static analysis of the query set against the repo's directory structure.

## Goals / Non-Goals

**Goals:**
- Compute f_k distribution from existing benchmark query sets by counting the depth of annotated relevant files
- Support aggregation across multiple repos and query categories (focused, module, cross-cutting)
- Output machine-readable TSV (compatible with existing harness) and human-readable summary statistics
- Produce concentration metrics that directly parameterize the paper's model: mean, std dev, entropy, support range

**Non-Goals:**
- Measuring f_k from actual routing runs (that conflates routing accuracy with depth distribution)
- Modifying the routing benchmark itself
- Automatic plot generation (analysis scripts can consume the TSV)
- Measuring branching factors B_l or accuracy alpha_l (separate future work)

## Decisions

### 1. Compute depth from file paths, not from SRT traversal

Depth of a relevant file is simply the number of path components (e.g., `pkg/teams/types.go` = depth 3). This requires no SRT index, no embeddings, and no LLM calls. It measures the ground-truth structural property of the repository.

**Alternative considered**: Walk the .sem/ tree to compute depth. Rejected because it couples depth measurement to whether the SRT has been built, and the depth is a property of the filesystem, not the index.

### 2. Implement in Rust CLI as `bench depth-profile` subcommand

The Rust CLI already has a `Bench` command. We add a new phase string `"depth-profile"` that reads a query YAML file, computes depths, and writes TSV rows using the existing `bench::append_tsv` infrastructure.

**Alternative considered**: Python-only implementation in `bench/depth_profile.py`. Rejected because the CLI is the primary entry point and Rust can parse the same YAML format. However, a Python analysis script can also be added for post-hoc statistics from the TSV.

### 3. Weight by graded relevance

Each relevant file has a `relevance` score (1-3). The f_k distribution should be relevance-weighted: a file with relevance=3 contributes 3x to the depth-k bin vs relevance=1. This matches the paper's model where f_k represents probability of the *relevant* item, and higher-relevance items are more likely targets.

**Alternative considered**: Unweighted (each file counts equally). We output both weighted and unweighted distributions so the analysis can use either.

### 4. TSV output format compatible with existing harness

Emit rows with `phase="depth-profile"` using the standard `(timestamp, phase, repo, system, query_id, control_json, metric, value)` format. Metrics emitted per query:
- `depth_mean`: mean depth of relevant files for this query
- `depth_max`: max depth
- `depth_min`: min depth

Aggregate metrics (across all queries in the file):
- `fk_d{N}`: fraction of relevance-weighted mass at depth N (the actual f_k values)
- `fk_mean`: overall mean depth
- `fk_std`: standard deviation
- `fk_entropy`: Shannon entropy of the distribution (measures concentration)
- `fk_support_min` / `fk_support_max`: range of depths with nonzero mass
- `repo_max_depth`: maximum depth in the repository tree (H in the paper)

### 5. Also measure repo structural properties

While computing f_k from queries, also walk the repo tree to measure H (max depth) and B_l (branching factors by level). These are needed by the paper's model and are cheap to compute alongside f_k.

## Risks / Trade-offs

- **[Small corpus]** Currently only one repo (fellowship) has a query set. f_k estimates from a single repo may not generalize. Mitigation: the turborepo query set is being added; the tool supports multiple repos and can aggregate across them.
- **[Relevance annotation bias]** f_k is only as good as the human-annotated relevant files in query YAMLs. If annotations systematically miss shallow or deep files, f_k is biased. Mitigation: document this limitation; future work can validate with actual routing runs.
- **[Path depth != SRT depth]** If the SRT prunes certain directory levels (e.g., skipping single-child directories), the effective routing depth could differ from the raw path depth. Mitigation: measure both raw and SRT-effective depth if .sem/ records exist.

## Open Questions

- Should f_k be normalized per-repo before averaging across repos, or computed from the pooled distribution? (Probably per-repo normalization to avoid large repos dominating.)
- Is there value in breaking f_k down by query category (focused vs module vs cross-cutting)?

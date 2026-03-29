use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;
use walkdir::WalkDir;

// --- Query YAML parsing (tasks 1.2, 1.3) ---

#[derive(Debug, Deserialize)]
pub struct QueryFile {
    pub queries: Vec<Query>,
}

#[derive(Debug, Deserialize)]
pub struct Query {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub relevant: Vec<RelevantFile>,
}

#[derive(Debug, Deserialize)]
pub struct RelevantFile {
    pub path: String,
    #[serde(default = "default_relevance")]
    pub relevance: u32,
}

fn default_relevance() -> u32 {
    1
}

pub fn load_queries(path: &Path) -> Result<Vec<Query>> {
    let content = std::fs::read_to_string(path)?;
    let qf: QueryFile = serde_yaml::from_str(&content)?;
    Ok(qf.queries)
}

// --- Depth computation (tasks 2.1-2.4) ---

pub fn file_depth(path: &str) -> usize {
    path.split('/').filter(|s| !s.is_empty()).count()
}

/// Compute the f_k depth distribution from relevant files.
/// If `weighted`, each file contributes its relevance score; otherwise 1.
pub fn compute_fk(queries: &[Query], weighted: bool) -> BTreeMap<usize, f64> {
    let mut counts: BTreeMap<usize, f64> = BTreeMap::new();
    let mut total = 0.0;

    for q in queries {
        for rf in &q.relevant {
            let d = file_depth(&rf.path);
            let w = if weighted { rf.relevance as f64 } else { 1.0 };
            *counts.entry(d).or_default() += w;
            total += w;
        }
    }

    if total > 0.0 {
        for v in counts.values_mut() {
            *v /= total;
        }
    }
    counts
}

#[derive(Debug)]
pub struct DepthStats {
    pub mean: f64,
    pub std_dev: f64,
    pub entropy: f64,
    pub support_min: usize,
    pub support_max: usize,
    pub num_queries: usize,
    pub num_files: usize,
}

pub fn compute_stats(fk: &BTreeMap<usize, f64>, num_queries: usize, num_files: usize) -> DepthStats {
    let mean: f64 = fk.iter().map(|(d, p)| *d as f64 * p).sum();

    let variance: f64 = fk.iter().map(|(d, p)| {
        let diff = *d as f64 - mean;
        diff * diff * p
    }).sum();

    let entropy: f64 = fk.iter()
        .filter(|(_, p)| **p > 0.0)
        .map(|(_, p)| -p * p.log2())
        .sum();

    let support_min = fk.keys().next().cloned().unwrap_or(0);
    let support_max = fk.keys().last().cloned().unwrap_or(0);

    DepthStats { mean, std_dev: variance.sqrt(), entropy, support_min, support_max, num_queries, num_files }
}

// --- Per-query metrics ---

pub struct QueryDepthMetrics {
    pub query_id: String,
    pub depth_mean: f64,
    pub depth_min: usize,
    pub depth_max: usize,
}

pub fn per_query_metrics(queries: &[Query]) -> Vec<QueryDepthMetrics> {
    queries.iter().filter_map(|q| {
        if q.relevant.is_empty() {
            return None;
        }
        let depths: Vec<usize> = q.relevant.iter().map(|rf| file_depth(&rf.path)).collect();
        let min = *depths.iter().min().unwrap();
        let max = *depths.iter().max().unwrap();
        let mean = depths.iter().sum::<usize>() as f64 / depths.len() as f64;
        Some(QueryDepthMetrics {
            query_id: q.id.clone(),
            depth_mean: mean,
            depth_min: min,
            depth_max: max,
        })
    }).collect()
}

// --- Repo tree structure (tasks 3.1-3.2) ---

/// Walk the repo tree and compute max depth and mean branching factor per level.
/// Respects standard ignore rules (dotfiles, dotdirs, common excludes).
pub fn repo_tree_metrics(repo_path: &Path) -> Result<(usize, BTreeMap<usize, f64>)> {
    let mut max_depth: usize = 0;
    let mut dir_children: BTreeMap<usize, BTreeMap<String, usize>> = BTreeMap::new();

    let skip_dirs = ["node_modules", "vendor", "dist", "build", "target",
                     "__pycache__", ".build", ".gradle"];

    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            if name.starts_with('.') && e.depth() > 0 {
                return false;
            }
            if e.file_type().is_dir() && skip_dirs.contains(&name.as_ref()) {
                return false;
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if entry.depth() == 0 {
            continue;
        }
        if entry.file_type().is_file() && entry.depth() > max_depth {
            max_depth = entry.depth();
        }
        let parent = entry.path().parent().unwrap_or(repo_path);
        let parent_key = parent.to_string_lossy().to_string();
        let parent_depth = entry.depth() - 1;
        *dir_children.entry(parent_depth)
            .or_default()
            .entry(parent_key)
            .or_default() += 1;
    }

    let mut branching: BTreeMap<usize, f64> = BTreeMap::new();
    for (level, dirs) in &dir_children {
        let counts: Vec<usize> = dirs.values().copied().collect();
        if !counts.is_empty() {
            let mean = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
            branching.insert(*level, mean);
        }
    }

    Ok((max_depth, branching))
}

// --- Category breakdown (task 7.1) ---

pub fn per_category_fk(queries: &[Query], weighted: bool) -> BTreeMap<String, BTreeMap<usize, f64>> {
    let mut by_cat: BTreeMap<String, Vec<&Query>> = BTreeMap::new();
    for q in queries {
        let cat = if q.category.is_empty() { "uncategorized" } else { &q.category };
        by_cat.entry(cat.to_string()).or_default().push(q);
    }

    by_cat.into_iter().map(|(cat, qs)| {
        // Recompute fk for this subset
        let owned: Vec<Query> = qs.into_iter().map(|q| Query {
            id: q.id.clone(),
            question: q.question.clone(),
            category: q.category.clone(),
            relevant: q.relevant.iter().map(|rf| RelevantFile {
                path: rf.path.clone(),
                relevance: rf.relevance,
            }).collect(),
        }).collect();
        let fk = compute_fk(&owned, weighted);
        (cat, fk)
    }).collect()
}

// --- Human-readable output (tasks 6.1-6.3) ---

pub fn print_histogram(fk: &BTreeMap<usize, f64>, label: &str) {
    eprintln!("\n  f_k distribution ({label}):");
    let max_bar = 40;
    let max_val = fk.values().cloned().fold(0.0f64, f64::max);
    for (&depth, &prob) in fk {
        let bar_len = if max_val > 0.0 { (prob / max_val * max_bar as f64) as usize } else { 0 };
        let bar: String = "█".repeat(bar_len);
        eprintln!("    d={depth:2}  {bar:<width$}  {prob:.4}", width = max_bar);
    }
}

pub fn print_stats(stats: &DepthStats) {
    eprintln!("  mean={:.2}  std={:.2}  entropy={:.3}  support=[{}, {}]  queries={}  files={}",
        stats.mean, stats.std_dev, stats.entropy,
        stats.support_min, stats.support_max,
        stats.num_queries, stats.num_files);
}

pub fn print_tree_metrics(max_depth: usize, branching: &BTreeMap<usize, f64>) {
    eprintln!("\n  Repo tree: H={max_depth}");
    for (&level, &bf) in branching {
        eprintln!("    B_{level} = {bf:.1}");
    }
}

// --- TSV emission (tasks 5.1-5.5) ---

type TsvRow = (String, String, String, String, String, String, String, f64);

fn now_ts() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

fn make_row(repo: &str, query_id: &str, ctrl: &str, metric: &str, value: f64) -> TsvRow {
    (now_ts(), "depth-profile".to_string(), repo.to_string(), "srt".to_string(),
     query_id.to_string(), ctrl.to_string(), metric.to_string(), value)
}

pub fn emit_rows(
    repo: &str,
    queries: &[Query],
    fk_weighted: &BTreeMap<usize, f64>,
    fk_unweighted: &BTreeMap<usize, f64>,
    stats_w: &DepthStats,
    stats_u: &DepthStats,
    max_depth: usize,
    branching: &BTreeMap<usize, f64>,
    cat_fk: &BTreeMap<String, BTreeMap<usize, f64>>,
) -> Vec<TsvRow> {
    let mut rows = Vec::new();

    // Per-query metrics (task 5.1)
    for m in per_query_metrics(queries) {
        rows.push(make_row(repo, &m.query_id, "", "depth_mean", m.depth_mean));
        rows.push(make_row(repo, &m.query_id, "", "depth_min", m.depth_min as f64));
        rows.push(make_row(repo, &m.query_id, "", "depth_max", m.depth_max as f64));
    }

    // Aggregate f_k (task 5.2) — weighted
    for (&d, &p) in fk_weighted {
        rows.push(make_row(repo, "", "", &format!("fk_d{d}"), p));
    }

    // Aggregate stats (task 5.3) — weighted
    rows.push(make_row(repo, "", "", "fk_mean", stats_w.mean));
    rows.push(make_row(repo, "", "", "fk_std", stats_w.std_dev));
    rows.push(make_row(repo, "", "", "fk_entropy", stats_w.entropy));
    rows.push(make_row(repo, "", "", "fk_support_min", stats_w.support_min as f64));
    rows.push(make_row(repo, "", "", "fk_support_max", stats_w.support_max as f64));
    rows.push(make_row(repo, "", "", "repo_max_depth", max_depth as f64));

    // Branching factors (task 5.4)
    for (&level, &bf) in branching {
        rows.push(make_row(repo, "", "", &format!("B_{level}"), bf));
    }

    // Unweighted variants (task 5.5)
    for (&d, &p) in fk_unweighted {
        rows.push(make_row(repo, "", "", &format!("unweighted_fk_d{d}"), p));
    }
    rows.push(make_row(repo, "", "", "unweighted_fk_mean", stats_u.mean));
    rows.push(make_row(repo, "", "", "unweighted_fk_std", stats_u.std_dev));
    rows.push(make_row(repo, "", "", "unweighted_fk_entropy", stats_u.entropy));
    rows.push(make_row(repo, "", "", "unweighted_fk_support_min", stats_u.support_min as f64));
    rows.push(make_row(repo, "", "", "unweighted_fk_support_max", stats_u.support_max as f64));

    // Per-category f_k (task 7.2)
    for (cat, fk) in cat_fk {
        for (&d, &p) in fk {
            rows.push(make_row(repo, "", cat, &format!("fk_d{d}"), p));
        }
    }

    rows
}

// --- Main entry point ---

pub fn run_depth_profile(
    repo_path: &Path,
    queries_path: &Path,
    results_path: &Path,
) -> Result<()> {
    let queries = load_queries(queries_path)?;
    let repo_name = repo_path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let num_files: usize = queries.iter().map(|q| q.relevant.len()).sum();
    let num_queries = queries.len();

    // Compute distributions
    let fk_w = compute_fk(&queries, true);
    let fk_u = compute_fk(&queries, false);
    let stats_w = compute_stats(&fk_w, num_queries, num_files);
    let stats_u = compute_stats(&fk_u, num_queries, num_files);

    // Repo tree structure
    let (max_depth, branching) = repo_tree_metrics(repo_path)?;

    // Per-category
    let cat_fk = per_category_fk(&queries, true);

    // Human-readable output
    eprintln!("\nDepth profile: {} queries, {} relevant files", num_queries, num_files);
    print_histogram(&fk_w, "weighted");
    print_stats(&stats_w);
    print_histogram(&fk_u, "unweighted");
    print_stats(&stats_u);
    print_tree_metrics(max_depth, &branching);

    // Per-category summary (task 7.3)
    for (cat, fk) in &cat_fk {
        print_histogram(fk, cat);
        let cat_queries: Vec<&Query> = queries.iter().filter(|q| {
            let c = if q.category.is_empty() { "uncategorized" } else { &q.category };
            c == cat
        }).collect();
        let nf: usize = cat_queries.iter().map(|q| q.relevant.len()).sum();
        let s = compute_stats(fk, cat_queries.len(), nf);
        print_stats(&s);
    }

    // TSV output
    let rows = emit_rows(&repo_name, &queries, &fk_w, &fk_u, &stats_w, &stats_u, max_depth, &branching, &cat_fk);
    crate::bench::append_tsv(results_path, &rows)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task 1.4: parse YAML
    #[test]
    fn test_parse_query_yaml() {
        let yaml = r#"
queries:
  - id: q01
    question: "where is Foo?"
    category: focused
    relevant:
      - path: pkg/foo.go
        relevance: 3
      - path: pkg/bar.go
        relevance: 1
  - id: q02
    question: "how does Bar work?"
    category: module
    relevant:
      - path: internal/bar/impl.go
        relevance: 2
"#;
        let qf: QueryFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(qf.queries.len(), 2);
        assert_eq!(qf.queries[0].id, "q01");
        assert_eq!(qf.queries[0].relevant.len(), 2);
        assert_eq!(qf.queries[0].relevant[0].relevance, 3);
        assert_eq!(qf.queries[1].category, "module");
    }

    // Task 2.5: depth computation
    #[test]
    fn test_file_depth() {
        assert_eq!(file_depth("main.go"), 1);
        assert_eq!(file_depth("pkg/teams/types.go"), 3);
        assert_eq!(file_depth("internal/tui/components/modal/view.go"), 5);
        assert_eq!(file_depth(""), 0);
    }

    #[test]
    fn test_fk_normalization() {
        let queries = vec![
            Query {
                id: "q1".into(), question: "".into(), category: "".into(),
                relevant: vec![
                    RelevantFile { path: "a/b.go".into(), relevance: 3 },
                    RelevantFile { path: "a/b/c.go".into(), relevance: 1 },
                ],
            },
            Query {
                id: "q2".into(), question: "".into(), category: "".into(),
                relevant: vec![
                    RelevantFile { path: "x/y.go".into(), relevance: 2 },
                ],
            },
        ];

        let fk = compute_fk(&queries, true);
        let sum: f64 = fk.values().sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // weighted: d2 = (3+2)/6 = 5/6, d3 = 1/6
        assert!((fk[&2] - 5.0 / 6.0).abs() < 1e-10);
        assert!((fk[&3] - 1.0 / 6.0).abs() < 1e-10);

        let fk_u = compute_fk(&queries, false);
        let sum_u: f64 = fk_u.values().sum();
        assert!((sum_u - 1.0).abs() < 1e-10);
        // unweighted: d2 = 2/3, d3 = 1/3
        assert!((fk_u[&2] - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_stats_known_distribution() {
        let mut fk = BTreeMap::new();
        fk.insert(3, 0.2);
        fk.insert(4, 0.6);
        fk.insert(5, 0.2);

        let stats = compute_stats(&fk, 10, 20);
        assert!((stats.mean - 4.0).abs() < 1e-10);
        assert_eq!(stats.support_min, 3);
        assert_eq!(stats.support_max, 5);
        assert!(stats.entropy > 0.0);
        assert!(stats.entropy < 2.0_f64.log2() * 3.0); // < log2(3)
    }

    // Task 3.3: repo tree metrics
    #[test]
    fn test_repo_tree_metrics() {
        let dir = tempfile::tempdir().unwrap();
        // Create: root/a/b/c.txt (depth 3)
        //         root/a/d.txt    (depth 2)
        //         root/e.txt      (depth 1)
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/c.txt"), "").unwrap();
        std::fs::write(dir.path().join("a/d.txt"), "").unwrap();
        std::fs::write(dir.path().join("e.txt"), "").unwrap();

        let (max_d, branching) = repo_tree_metrics(dir.path()).unwrap();
        assert_eq!(max_d, 3);
        // Level 0 (root): 2 children (a/, e.txt)
        assert!((branching[&0] - 2.0).abs() < 1e-10);
        // Level 1 (a/): 2 children (b/, d.txt)
        assert!((branching[&1] - 2.0).abs() < 1e-10);
        // Level 2 (b/): 1 child (c.txt)
        assert!((branching[&2] - 1.0).abs() < 1e-10);
    }
}

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::embedder;
use crate::records::{self, SEM_DIR, DIR_RECORD};
use crate::vec_store;
use crate::depth_profile;

/// Append rows to a TSV results file, writing a header if the file is new.
pub fn append_tsv(
    path: &Path,
    rows: &[(String, String, String, String, String, String, String, f64)],
) -> Result<()> {
    let write_header = !path.exists();
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    if write_header {
        writeln!(
            file,
            "timestamp\tphase\trepo\tsystem\tquery_id\tcontrol_json\tmetric\tvalue"
        )?;
    }
    for (ts, phase, repo, system, qid, ctrl, metric, value) in rows {
        writeln!(
            file,
            "{ts}\t{phase}\t{repo}\t{system}\t{qid}\t{ctrl}\t{metric}\t{value}"
        )?;
    }
    Ok(())
}

/// Run structural quality checks on all .sem/ records in a repo.
///
/// Returns a vec of (metric_name, value) pairs:
/// - frontmatter_errors: count of records missing required fields
/// - orphan_records: count of records whose source file/dir doesn't exist
/// - children_coverage: average fraction of children mentioned in directory summaries
pub fn run_quality(repo_path: &Path, _repo_name: &str) -> Result<Vec<(String, f64)>> {
    let mut frontmatter_errors: u64 = 0;
    let mut orphan_count: u64 = 0;
    let mut coverage_scores: Vec<f64> = Vec::new();

    // Walk all .sem/*.md records
    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only look at .md files inside .sem/ directories
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let parent = match path.parent() {
            Some(p) => p,
            None => continue,
        };
        if parent.file_name().and_then(|n| n.to_str()) != Some(SEM_DIR) {
            continue;
        }

        let record = match records::read_record(path)? {
            Some(r) => r,
            None => {
                frontmatter_errors += 1;
                continue;
            }
        };

        // Check required fields are non-empty
        if record.path.is_empty() || record.node_type.is_empty() || record.content_hash.is_empty() {
            frontmatter_errors += 1;
        }

        // Check type validity
        if record.node_type != "file" && record.node_type != "directory" {
            frontmatter_errors += 1;
        }

        // Orphan check: does the source exist?
        if record.node_type == "file" {
            let source = repo_path.join(&record.path);
            if !source.exists() {
                orphan_count += 1;
            }
        } else if record.node_type == "directory" {
            let dir_path = if record.path.is_empty() || record.path == "." {
                repo_path.to_path_buf()
            } else {
                repo_path.join(&record.path)
            };
            if !dir_path.is_dir() {
                orphan_count += 1;
            }
        }

        // Children coverage for directory records
        if record.node_type == "directory" {
            let dir_path = if record.path.is_empty() || record.path == "." {
                repo_path.to_path_buf()
            } else {
                repo_path.join(&record.path)
            };
            let sem_dir = dir_path.join(SEM_DIR);
            if sem_dir.is_dir() {
                // Find bold-mentioned names in the summary (manual parse for **name**)
                let mentioned: std::collections::HashSet<String> = {
                    let mut set = std::collections::HashSet::new();
                    let s = &record.summary;
                    let mut start = 0;
                    while let Some(open) = s[start..].find("**") {
                        let open_abs = start + open + 2;
                        if let Some(close) = s[open_abs..].find("**") {
                            let name = &s[open_abs..open_abs + close];
                            if !name.is_empty() {
                                set.insert(name.to_string());
                            }
                            start = open_abs + close + 2;
                        } else {
                            break;
                        }
                    }
                    set
                };

                // Find child record names (excluding __dir__.md)
                let child_names: Vec<String> = fs::read_dir(&sem_dir)?
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name == "__dir__.md" || !name.ends_with(".md") {
                            None
                        } else {
                            Some(name.trim_end_matches(".md").to_string())
                        }
                    })
                    .collect();

                if !child_names.is_empty() {
                    let found = child_names
                        .iter()
                        .filter(|c| mentioned.contains(c.as_str()))
                        .count();
                    coverage_scores.push(found as f64 / child_names.len() as f64);
                }
            }
        }
    }

    let avg_coverage = if coverage_scores.is_empty() {
        1.0
    } else {
        coverage_scores.iter().sum::<f64>() / coverage_scores.len() as f64
    };

    Ok(vec![
        ("children_coverage".to_string(), avg_coverage),
        ("frontmatter_errors".to_string(), frontmatter_errors as f64),
        ("orphan_records".to_string(), orphan_count as f64),
    ])
}

// ---------------------------------------------------------------------------
// Diagnostics: centroid fidelity, per-level entropy, query-relative SNR
// ---------------------------------------------------------------------------

/// Per-directory diagnostics collected during the walk.
struct DirDiag {
    /// Repo-relative path of the directory
    rel_path: String,
    /// Depth (count of '/' segments; root = 0)
    depth: usize,
    /// Centroid fidelity: mean cosine sim between dir embedding and child embeddings
    fidelity: f32,
    /// Child-to-child cosine similarities (for variance / entropy computation)
    child_sims: Vec<f32>,
}

/// Walk all .sem/ directories, load embeddings, compute per-directory centroid fidelity.
fn collect_dir_diagnostics(repo_path: &Path) -> Result<Vec<DirDiag>> {
    let mut diags = Vec::new();

    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        // We only care about __dir__.md files inside .sem/ directories
        if path.file_name().and_then(|n| n.to_str()) != Some(DIR_RECORD) {
            continue;
        }
        let sem_dir = match path.parent() {
            Some(p) => p,
            None => continue,
        };
        if sem_dir.file_name().and_then(|n| n.to_str()) != Some(SEM_DIR) {
            continue;
        }
        let actual_dir = match sem_dir.parent() {
            Some(p) => p,
            None => continue,
        };

        // Load directory embedding
        let dir_vec_path = sem_dir.join("__dir__.vec");
        let dir_vec = match vec_store::read_vec(&dir_vec_path)? {
            Some(v) => v,
            None => continue, // no embedding, skip
        };

        // Collect child .vec files (everything in .sem/ except __dir__.vec)
        let mut child_vecs: Vec<(String, Vec<f32>)> = Vec::new();
        for child_entry in fs::read_dir(sem_dir)?.filter_map(|e| e.ok()) {
            let child_name = child_entry.file_name().to_string_lossy().to_string();
            if !child_name.ends_with(".vec") || child_name == "__dir__.vec" {
                continue;
            }
            if let Some(data) = vec_store::read_vec(&child_entry.path())? {
                child_vecs.push((child_name, data.vector));
            }
        }

        if child_vecs.is_empty() {
            continue;
        }

        // Centroid fidelity: mean cosine sim(dir_embedding, child_embedding)
        let fidelity: f32 = child_vecs
            .iter()
            .map(|(_, v)| embedder::cosine_similarity(&dir_vec.vector, v))
            .sum::<f32>()
            / child_vecs.len() as f32;

        // Pairwise cosine similarities among siblings (for spread/entropy)
        let mut child_sims = Vec::new();
        for i in 0..child_vecs.len() {
            for j in (i + 1)..child_vecs.len() {
                child_sims.push(embedder::cosine_similarity(&child_vecs[i].1, &child_vecs[j].1));
            }
        }

        // Compute depth from repo-relative path
        let rel_path = actual_dir
            .strip_prefix(repo_path)
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .to_string();
        let depth = if rel_path.is_empty() {
            0
        } else {
            rel_path.split('/').filter(|s| !s.is_empty()).count()
        };

        diags.push(DirDiag {
            rel_path,
            depth,
            fidelity,
            child_sims,
        });
    }

    Ok(diags)
}

/// Run SRT diagnostics: centroid fidelity, per-level entropy, and optionally query-relative SNR.
pub fn run_diagnostics(
    repo_path: &Path,
    queries_path: Option<&Path>,
    results_path: &Path,
) -> Result<()> {
    let diags = collect_dir_diagnostics(repo_path)?;
    if diags.is_empty() {
        eprintln!("No directory embeddings found. Run `semtree build` and `semtree embed` first.");
        return Ok(());
    }

    // --- 1. Per-directory centroid fidelity ---
    eprintln!("=== Centroid Fidelity ρ(v) ===");
    eprintln!("{:<50} {:>5} {:>8}", "directory", "depth", "ρ(v)");
    eprintln!("{}", "-".repeat(65));
    for d in &diags {
        let name = if d.rel_path.is_empty() { "." } else { &d.rel_path };
        eprintln!("{:<50} {:>5} {:>8.4}", name, d.depth, d.fidelity);
    }

    // --- 2. Per-level aggregation ---
    let max_depth = diags.iter().map(|d| d.depth).max().unwrap_or(0);
    eprintln!("\n=== Per-Level Summary ===");
    eprintln!("{:>5} {:>6} {:>10} {:>10} {:>10}", "depth", "dirs", "mean_ρ", "spread_μ", "spread_σ²");
    eprintln!("{}", "-".repeat(45));

    let now = {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        format!("{}", d.as_secs())
    };
    let mut tsv_rows = Vec::new();

    for depth in 0..=max_depth {
        let at_depth: Vec<&DirDiag> = diags.iter().filter(|d| d.depth == depth).collect();
        if at_depth.is_empty() {
            continue;
        }
        let n = at_depth.len() as f64;
        let mean_fidelity = at_depth.iter().map(|d| d.fidelity as f64).sum::<f64>() / n;

        // Mean sibling spread and variance
        let all_sims: Vec<f32> = at_depth.iter().flat_map(|d| d.child_sims.iter().copied()).collect();
        let (spread_mean, spread_var) = if all_sims.is_empty() {
            (0.0, 0.0)
        } else {
            let m = all_sims.iter().copied().sum::<f32>() / all_sims.len() as f32;
            let v = all_sims.iter().map(|s| (s - m).powi(2)).sum::<f32>() / all_sims.len() as f32;
            (m as f64, v as f64)
        };

        eprintln!("{:>5} {:>6} {:>10.4} {:>10.4} {:>10.6}", depth, at_depth.len(), mean_fidelity, spread_mean, spread_var);

        tsv_rows.push((now.clone(), "diagnostics".to_string(), "local".to_string(), "srt".to_string(),
                        String::new(), format!("{{\"depth\":{depth}}}"),
                        "centroid_fidelity".to_string(), mean_fidelity));
        tsv_rows.push((now.clone(), "diagnostics".to_string(), "local".to_string(), "srt".to_string(),
                        String::new(), format!("{{\"depth\":{depth}}}"),
                        "sibling_spread_mean".to_string(), spread_mean));
        tsv_rows.push((now.clone(), "diagnostics".to_string(), "local".to_string(), "srt".to_string(),
                        String::new(), format!("{{\"depth\":{depth}}}"),
                        "sibling_spread_var".to_string(), spread_var));
    }

    // Global summary
    let global_fidelity = diags.iter().map(|d| d.fidelity as f64).sum::<f64>() / diags.len() as f64;
    eprintln!("\nGlobal mean ρ(v): {:.4} ({} directories)", global_fidelity, diags.len());
    tsv_rows.push((now.clone(), "diagnostics".to_string(), "local".to_string(), "srt".to_string(),
                    String::new(), String::new(),
                    "global_centroid_fidelity".to_string(), global_fidelity));

    // --- 3. Query-relative SNR (if queries provided) ---
    if let Some(qpath) = queries_path {
        let queries = depth_profile::load_queries(qpath)?;
        eprintln!("\n=== Query-Relative SNR ===");
        eprintln!("Loaded {} queries from {}", queries.len(), qpath.display());

        // Find model from an existing .vec file
        let embed_model = find_embed_model(repo_path)?
            .unwrap_or_else(|| "BAAI/bge-small-en-v1.5".to_string());

        eprintln!("Using embedding model: {}", embed_model);
        eprintln!("{:<8} {:>5} {:>10} {:>10} {:>10}", "query", "depth", "sim_rel", "sim_irr", "SNR");
        eprintln!("{}", "-".repeat(50));

        // Build a map from directory path -> child vec data for quick lookup
        let dir_children = build_dir_children_map(repo_path)?;

        for q in &queries {
            let query_vec = embedder::embed_query(&q.question, &embed_model)?;

            // Collect all ancestor directory paths for relevant files
            let relevant_paths: std::collections::HashSet<String> = q.relevant
                .iter()
                .map(|r| r.path.clone())
                .collect();

            // Find directories on the path to relevant files
            let mut dir_relevance: std::collections::HashMap<String, (Vec<String>, Vec<String>)> =
                std::collections::HashMap::new();

            for (dir_path, children) in &dir_children {
                let mut relevant_children = Vec::new();
                let mut irrelevant_children = Vec::new();

                for (child_name, _) in children {
                    // Reconstruct child repo-relative path
                    let child_rel = if dir_path.is_empty() {
                        child_name.clone()
                    } else {
                        format!("{}/{}", dir_path, child_name)
                    };

                    // Check if this child or anything in its subtree is relevant
                    let is_relevant = relevant_paths.iter().any(|rp| {
                        rp == &child_rel || rp.starts_with(&format!("{}/", child_rel))
                    });

                    if is_relevant {
                        relevant_children.push(child_name.clone());
                    } else {
                        irrelevant_children.push(child_name.clone());
                    }
                }

                if !relevant_children.is_empty() && !irrelevant_children.is_empty() {
                    dir_relevance.insert(dir_path.clone(), (relevant_children, irrelevant_children));
                }
            }

            // Compute SNR at each directory on the path
            for (dir_path, (rel_names, irr_names)) in &dir_relevance {
                let children = dir_children.get(dir_path.as_str()).unwrap();
                let child_map: std::collections::HashMap<&str, &Vec<f32>> = children
                    .iter()
                    .map(|(name, vec)| (name.as_str(), vec))
                    .collect();

                let sim_relevant: f32 = rel_names
                    .iter()
                    .filter_map(|n| child_map.get(n.as_str()))
                    .map(|v| embedder::cosine_similarity(&query_vec, v))
                    .sum::<f32>()
                    / rel_names.len().max(1) as f32;

                let sim_irrelevant: f32 = irr_names
                    .iter()
                    .filter_map(|n| child_map.get(n.as_str()))
                    .map(|v| embedder::cosine_similarity(&query_vec, v))
                    .sum::<f32>()
                    / irr_names.len().max(1) as f32;

                let snr = if sim_irrelevant > 0.0 {
                    sim_relevant as f64 / sim_irrelevant as f64
                } else {
                    f64::INFINITY
                };

                let depth = if dir_path.is_empty() {
                    0
                } else {
                    dir_path.split('/').filter(|s| !s.is_empty()).count()
                };

                eprintln!("{:<8} {:>5} {:>10.4} {:>10.4} {:>10.4}", q.id, depth, sim_relevant, sim_irrelevant, snr);

                tsv_rows.push((now.clone(), "diagnostics".to_string(), "local".to_string(), "srt".to_string(),
                               q.id.clone(), format!("{{\"depth\":{depth},\"dir\":\"{dir_path}\"}}"),
                               "snr".to_string(), snr));
                tsv_rows.push((now.clone(), "diagnostics".to_string(), "local".to_string(), "srt".to_string(),
                               q.id.clone(), format!("{{\"depth\":{depth},\"dir\":\"{dir_path}\"}}"),
                               "sim_relevant".to_string(), sim_relevant as f64));
                tsv_rows.push((now.clone(), "diagnostics".to_string(), "local".to_string(), "srt".to_string(),
                               q.id.clone(), format!("{{\"depth\":{depth},\"dir\":\"{dir_path}\"}}"),
                               "sim_irrelevant".to_string(), sim_irrelevant as f64));
            }
        }
    }

    // Write all TSV rows
    if !tsv_rows.is_empty() {
        append_tsv(results_path, &tsv_rows)?;
        eprintln!("\nAppended {} rows to {}", tsv_rows.len(), results_path.display());
    }

    Ok(())
}

/// Find the embedding model used in existing .vec files.
fn find_embed_model(repo_path: &Path) -> Result<Option<String>> {
    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("vec") {
            if let Some(data) = vec_store::read_vec(path)? {
                return Ok(Some(data.model));
            }
        }
    }
    Ok(None)
}

/// Build a map of dir_rel_path -> [(child_name, child_vec)] for all .sem/ directories.
fn build_dir_children_map(repo_path: &Path) -> Result<std::collections::HashMap<String, Vec<(String, Vec<f32>)>>> {
    let mut map: std::collections::HashMap<String, Vec<(String, Vec<f32>)>> = std::collections::HashMap::new();

    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) != Some(DIR_RECORD) {
            continue;
        }
        let sem_dir = match path.parent() {
            Some(p) => p,
            None => continue,
        };
        if sem_dir.file_name().and_then(|n| n.to_str()) != Some(SEM_DIR) {
            continue;
        }
        let actual_dir = match sem_dir.parent() {
            Some(p) => p,
            None => continue,
        };
        let rel_path = actual_dir
            .strip_prefix(repo_path)
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .to_string();

        let mut children = Vec::new();
        for child_entry in fs::read_dir(sem_dir)?.filter_map(|e| e.ok()) {
            let child_name = child_entry.file_name().to_string_lossy().to_string();
            if !child_name.ends_with(".vec") || child_name == "__dir__.vec" {
                continue;
            }
            if let Some(data) = vec_store::read_vec(&child_entry.path())? {
                // Strip .vec extension, then strip .md if it was e.g. "foo.rs.md.vec" -> "foo.rs"
                let name = child_name.trim_end_matches(".vec");
                // The .vec file for "foo.rs" is "foo.rs.vec" (sibling of "foo.rs.md")
                let name = name.trim_end_matches(".md");
                children.push((name.to_string(), data.vector));
            }
        }

        map.insert(rel_path, children);
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_append_tsv_creates_header() {
        let dir = tempdir().unwrap();
        let tsv = dir.path().join("results.tsv");

        let rows = vec![(
            "123".to_string(),
            "quality".to_string(),
            "repo".to_string(),
            "srt".to_string(),
            "".to_string(),
            "".to_string(),
            "metric".to_string(),
            1.0,
        )];
        append_tsv(&tsv, &rows).unwrap();

        let content = fs::read_to_string(&tsv).unwrap();
        assert!(content.starts_with("timestamp\tphase\t"));
        assert!(content.contains("123\tquality\trepo\tsrt\t\t\tmetric\t1"));
    }

    #[test]
    fn test_append_tsv_no_duplicate_header() {
        let dir = tempdir().unwrap();
        let tsv = dir.path().join("results.tsv");

        let rows = vec![(
            "1".to_string(),
            "q".to_string(),
            "r".to_string(),
            "s".to_string(),
            "".to_string(),
            "".to_string(),
            "m".to_string(),
            0.5,
        )];
        append_tsv(&tsv, &rows).unwrap();
        append_tsv(&tsv, &rows).unwrap();

        let content = fs::read_to_string(&tsv).unwrap();
        let header_count = content.matches("timestamp\tphase\t").count();
        assert_eq!(header_count, 1);
    }

    #[test]
    fn test_run_quality_empty_repo() {
        let dir = tempdir().unwrap();
        let metrics = run_quality(dir.path(), "test").unwrap();
        assert_eq!(metrics.len(), 3);
        // No records means defaults
        assert_eq!(metrics[0], ("children_coverage".to_string(), 1.0));
        assert_eq!(metrics[1], ("frontmatter_errors".to_string(), 0.0));
        assert_eq!(metrics[2], ("orphan_records".to_string(), 0.0));
    }

    #[test]
    fn test_run_quality_detects_orphan() {
        let dir = tempdir().unwrap();
        let sem = dir.path().join(SEM_DIR);
        fs::create_dir_all(&sem).unwrap();

        // Write a record pointing to a file that doesn't exist
        crate::records::write_record(
            &sem.join("ghost.rs.md"),
            "ghost.rs",
            "file",
            "abc123",
            "A ghost file.",
        )
        .unwrap();

        let metrics = run_quality(dir.path(), "test").unwrap();
        let orphans = metrics.iter().find(|(m, _)| m == "orphan_records").unwrap();
        assert_eq!(orphans.1, 1.0);
    }
}

//! Embedding-assisted routing: fastembed via Python subprocess, cosine ranking, directory operations.

use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context, Result};

use crate::records::{self, SEM_DIR};
use crate::vec_store;

/// Stats returned by embed_directory.
#[derive(Debug)]
pub struct EmbedStats {
    pub embedded: usize,
    pub skipped: usize,
    pub errored: usize,
}

/// A single level in the route descent.
#[derive(Debug)]
pub struct RouteLevel {
    pub dir: String,
    pub selected: Vec<(String, f32, String)>, // (path, score, first_line)
    pub all_children: usize,
    pub elapsed_ms: u64,
}

// ---------------------------------------------------------------------------
// Core embedding via Python subprocess
// ---------------------------------------------------------------------------

/// Embed a batch of texts via fastembed Python subprocess.
pub fn embed_texts(texts: &[String], model: &str) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }
    embed_via_python(texts, false, model)
}

/// Embed a single query string via fastembed Python subprocess.
pub fn embed_query(query: &str, model: &str) -> Result<Vec<f32>> {
    let results = embed_via_python(&[query.to_string()], true, model)?;
    results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty embedding result"))
}

fn embed_via_python(texts: &[String], is_query: bool, model: &str) -> Result<Vec<Vec<f32>>> {
    // Write texts to a temp file as JSON
    let mut tmpfile = tempfile::NamedTempFile::new().context("creating temp file for embedder")?;
    let json_texts = serde_json::to_string(texts)?;
    tmpfile.write_all(json_texts.as_bytes())?;
    tmpfile.flush()?;
    let tmp_path = tmpfile.path().to_string_lossy().to_string();

    let method = if is_query { "query_embed" } else { "passage_embed" };

    let script = format!(
        r#"
import json, sys
from fastembed import TextEmbedding
texts = json.load(open(sys.argv[1]))
model = TextEmbedding(model_name=sys.argv[2])
method = sys.argv[3]
if method == "query_embed":
    vecs = [list(v) for v in model.query_embed(texts)]
else:
    vecs = [list(v) for v in model.passage_embed(texts)]
# Output as JSON array of arrays
json.dump([[float(x) for x in v] for v in vecs], sys.stdout)
"#
    );

    let output = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .arg(&tmp_path)
        .arg(model)
        .arg(method)
        .output()
        .context("failed to run python3 for embedding")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("python embedder failed: {}", stderr);
    }

    let vectors: Vec<Vec<f32>> = serde_json::from_slice(&output.stdout)
        .context("parsing embedding output from python")?;

    Ok(vectors)
}

// ---------------------------------------------------------------------------
// Cosine similarity (native Rust)
// ---------------------------------------------------------------------------

/// Rank children by cosine similarity to query vector. Returns [(path, score)] descending.
pub fn cosine_rank(query_vec: &[f32], children: &[(&str, &[f32])]) -> Vec<(String, f32)> {
    let q_norm = dot(query_vec, query_vec).sqrt();
    if q_norm == 0.0 {
        return children
            .iter()
            .map(|(p, _)| (p.to_string(), 0.0))
            .collect();
    }

    let mut results: Vec<(String, f32)> = children
        .iter()
        .map(|(path, vec)| {
            let c_norm = dot(vec, vec).sqrt();
            let score = if c_norm == 0.0 {
                0.0
            } else {
                dot(query_vec, vec) / (q_norm * c_norm)
            };
            (path.to_string(), score)
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// ---------------------------------------------------------------------------
// Directory operations
// ---------------------------------------------------------------------------

/// Embed all .sem/ records under target_path. Returns stats.
pub fn embed_directory(target: &Path, model: &str, force: bool) -> Result<EmbedStats> {
    let pairs = find_sem_records(target)?;
    let mut stats = EmbedStats {
        embedded: 0,
        skipped: 0,
        errored: 0,
    };

    // Collect records that need embedding
    let mut to_embed: Vec<(std::path::PathBuf, String, String)> = Vec::new(); // (vec_path, content_hash, summary)

    for (md_path, vec_path) in &pairs {
        let record = match records::read_record(md_path)? {
            Some(r) => r,
            None => {
                stats.errored += 1;
                continue;
            }
        };

        if !force {
            if let Some(existing) = vec_store::read_vec(vec_path)? {
                if vec_store::is_vec_fresh(Some(&existing), &record.content_hash, model) {
                    stats.skipped += 1;
                    continue;
                }
            }
        }

        to_embed.push((vec_path.clone(), record.content_hash.clone(), record.summary.clone()));
    }

    if to_embed.is_empty() {
        return Ok(stats);
    }

    eprintln!("Embedding {} records...", to_embed.len());

    // Batch embed all summaries
    let texts: Vec<String> = to_embed.iter().map(|(_, _, s)| s.clone()).collect();
    let vectors = embed_texts(&texts, model)?;

    for ((vec_path, content_hash, _), vector) in to_embed.iter().zip(vectors.iter()) {
        vec_store::write_vec(vec_path, model, content_hash, vector)?;
        stats.embedded += 1;
    }

    Ok(stats)
}

/// Query children of a directory by cosine similarity.
///
/// Returns [(score, repo_relative_path, summary_first_line)] ranked descending.
/// Only considers immediate children of target_path (files in target_path/.sem/).
pub fn query_directory(
    target: &Path,
    query: &str,
    model: &str,
    top_k: Option<usize>,
    threshold: Option<f32>,
) -> Result<Vec<(f32, String, String)>> {
    let sem_dir = target.join(SEM_DIR);
    if !sem_dir.is_dir() {
        return Ok(vec![]);
    }

    // Load child vectors and records
    let mut children_vecs: Vec<(String, Vec<f32>)> = Vec::new();
    let mut children_summaries: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    let mut vec_entries: Vec<_> = std::fs::read_dir(&sem_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "vec")
        })
        .collect();
    vec_entries.sort_by_key(|e| e.path());

    for entry in vec_entries {
        let vec_path = entry.path();
        let vec_data = match vec_store::read_vec(&vec_path)? {
            Some(d) => d,
            None => continue,
        };

        let md_path = vec_path.with_extension("md");
        let record = match records::read_record(&md_path)? {
            Some(r) => r,
            None => continue,
        };

        let first_line = record
            .summary
            .split('\n')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        children_vecs.push((record.path.clone(), vec_data.vector));
        children_summaries.insert(record.path.clone(), first_line);
    }

    if children_vecs.is_empty() {
        return Ok(vec![]);
    }

    let query_vec = embed_query(query, model)?;

    let children_refs: Vec<(&str, &[f32])> = children_vecs
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_slice()))
        .collect();
    let ranked = cosine_rank(&query_vec, &children_refs);

    let mut results = Vec::new();
    for (path, score) in ranked {
        if let Some(t) = threshold {
            if score < t {
                continue;
            }
        }
        let first_line = children_summaries
            .get(&path)
            .cloned()
            .unwrap_or_default();
        results.push((score, path, first_line));
    }

    if let Some(k) = top_k {
        results.truncate(k);
    }

    Ok(results)
}

/// Full top-down descent from target_path, ranking children at each level.
///
/// At each level:
/// 1. Read .sem/*.vec files for immediate children
/// 2. Rank by cosine similarity to query
/// 3. Select top beam_width children
/// 4. If selected child is a directory, queue it for descent at next level
/// 5. If selected child is a file, add to candidates
/// 6. Stop when max_depth reached or no more directories to descend
pub fn route_directory(
    target: &Path,
    query: &str,
    model: &str,
    beam_width: usize,
    max_depth: usize,
) -> Result<Vec<RouteLevel>> {
    let query_vec = embed_query(query, model)?;

    let mut levels: Vec<RouteLevel> = Vec::new();
    // Queue: (directory_absolute_path, dir_relative_path, depth)
    let mut queue: Vec<(std::path::PathBuf, String, usize)> =
        vec![(target.to_path_buf(), String::new(), 0)];

    while let Some((dir_abs, dir_rel, depth)) = queue.first().cloned() {
        queue.remove(0);
        if depth >= max_depth {
            continue;
        }

        let level_start = Instant::now();
        let sem_dir = dir_abs.join(SEM_DIR);
        if !sem_dir.is_dir() {
            continue;
        }

        // Load child vectors and metadata
        let mut children_vecs: Vec<(String, Vec<f32>)> = Vec::new();
        let mut children_meta: std::collections::HashMap<String, (String, bool)> =
            std::collections::HashMap::new();

        let mut vec_entries: Vec<_> = std::fs::read_dir(&sem_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let p = e.path();
                p.extension().map_or(false, |ext| ext == "vec")
                    && p.file_stem().map_or(false, |s| s != "__dir__")
            })
            .collect();
        vec_entries.sort_by_key(|e| e.path());

        for entry in vec_entries {
            let vec_path = entry.path();
            let vec_data = match vec_store::read_vec(&vec_path)? {
                Some(d) => d,
                None => continue,
            };

            let md_path = vec_path.with_extension("md");
            let record = match records::read_record(&md_path)? {
                Some(r) => r,
                None => continue,
            };

            let first_line = record
                .summary
                .split('\n')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let is_dir = record.node_type == "directory";

            children_vecs.push((record.path.clone(), vec_data.vector));
            children_meta.insert(record.path.clone(), (first_line, is_dir));
        }

        if children_vecs.is_empty() {
            continue;
        }

        let children_refs: Vec<(&str, &[f32])> = children_vecs
            .iter()
            .map(|(p, v)| (p.as_str(), v.as_slice()))
            .collect();
        let ranked = cosine_rank(&query_vec, &children_refs);
        let selected: Vec<_> = ranked.iter().take(beam_width).cloned().collect();

        let level_info = RouteLevel {
            dir: if dir_rel.is_empty() {
                ".".to_string()
            } else {
                dir_rel.clone()
            },
            selected: selected
                .iter()
                .map(|(path, score)| {
                    let (first_line, _) = children_meta.get(path).cloned().unwrap_or_default();
                    (path.clone(), *score, first_line)
                })
                .collect(),
            all_children: ranked.len(),
            elapsed_ms: level_start.elapsed().as_millis() as u64,
        };
        levels.push(level_info);

        // Queue directory children for descent
        for (child_path, _score) in &selected {
            if let Some((_, is_dir)) = children_meta.get(child_path) {
                if *is_dir {
                    let child_abs = target.join(child_path);
                    if child_abs.is_dir() {
                        queue.push((child_abs, child_path.clone(), depth + 1));
                    }
                }
            }
        }
    }

    Ok(levels)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find all (.md record, .vec sidecar) pairs under target_path.
fn find_sem_records(target: &Path) -> Result<Vec<(std::path::PathBuf, std::path::PathBuf)>> {
    let mut pairs = Vec::new();

    for entry in walkdir::WalkDir::new(target)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path
            .parent()
            .and_then(|p| p.file_name())
            .map_or(false, |n| n == SEM_DIR)
            && path.extension().map_or(false, |e| e == "md")
        {
            let vec_path = path.with_extension("vec");
            pairs.push((path.to_path_buf(), vec_path));
        }
    }

    pairs.sort();
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_rank_basic() {
        let query = vec![1.0, 0.0, 0.0];
        let va = vec![1.0, 0.0, 0.0];
        let vb = vec![0.0, 1.0, 0.0];
        let vc = vec![0.707, 0.707, 0.0];
        let children: Vec<(&str, &[f32])> = vec![
            ("a", va.as_slice()),
            ("b", vb.as_slice()),
            ("c", vc.as_slice()),
        ];

        let ranked = cosine_rank(&query, &children);
        assert_eq!(ranked[0].0, "a");
        assert!((ranked[0].1 - 1.0).abs() < 1e-5);
        assert_eq!(ranked[1].0, "c");
        assert_eq!(ranked[2].0, "b");
        assert!((ranked[2].1 - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_rank_zero_query() {
        let query = vec![0.0, 0.0, 0.0];
        let va = vec![1.0, 0.0, 0.0];
        let children: Vec<(&str, &[f32])> = vec![("a", va.as_slice())];
        let ranked = cosine_rank(&query, &children);
        assert_eq!(ranked[0].1, 0.0);
    }

    #[test]
    fn test_cosine_rank_empty() {
        let query = vec![1.0, 0.0];
        let children: Vec<(&str, &[f32])> = vec![];
        let ranked = cosine_rank(&query, &children);
        assert!(ranked.is_empty());
    }

    #[test]
    fn test_dot_product() {
        assert!((dot(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]) - 32.0).abs() < 1e-5);
        assert!((dot(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 1e-5);
    }
}

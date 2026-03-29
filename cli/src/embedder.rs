//! Embedding-assisted routing: native fastembed inference, cosine ranking, directory operations.

use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

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
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RouteLevel {
    pub dir: String,
    pub selected: Vec<(String, f32, String)>, // (path, score, first_line)
    pub all_children: usize,
    pub elapsed_ms: u64,
}

// ---------------------------------------------------------------------------
// Native embedding via fastembed (ONNX Runtime)
// ---------------------------------------------------------------------------

static MODEL: Mutex<Option<TextEmbedding>> = Mutex::new(None);

fn get_model(model_name: &str) -> Result<std::sync::MutexGuard<'static, Option<TextEmbedding>>> {
    let mut guard = MODEL.lock().map_err(|e| anyhow::anyhow!("model lock poisoned: {e}"))?;
    if guard.is_none() {
        let model_enum = match model_name {
            "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            other => bail!("Unsupported embedding model: {other}. Only BAAI/bge-small-en-v1.5 is currently supported."),
        };
        let options = InitOptions::new(model_enum).with_show_download_progress(true);
        let model = TextEmbedding::try_new(options).context("loading embedding model")?;
        *guard = Some(model);
    }
    Ok(guard)
}

/// Embed a batch of document texts. Returns list of float vectors.
pub fn embed_texts(texts: &[String], model: &str) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }
    let guard = get_model(model)?;
    let m = guard.as_ref().unwrap();
    let embeddings = m.embed(texts.to_vec(), None)?;
    Ok(embeddings)
}

/// Embed a single query string. Returns a float vector.
pub fn embed_query(query: &str, model: &str) -> Result<Vec<f32>> {
    let guard = get_model(model)?;
    let m = guard.as_ref().unwrap();
    let embeddings = m.embed(vec![format!("query: {query}")], None)?;
    embeddings
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty embedding result"))
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

#[inline(always)]
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

        // For directory siblings, use the full __dir__.md content (with ## Children
        // routing table) for embedding instead of the abbreviated sibling summary.
        let embed_text = if record.node_type == "directory" && !md_path.ends_with(records::DIR_RECORD) {
            let dir_record = target.join(&record.path).join(SEM_DIR).join(records::DIR_RECORD);
            if let Ok(Some(dir_rec)) = records::read_record(&dir_record) {
                dir_rec.summary
            } else {
                record.summary.clone()
            }
        } else {
            record.summary.clone()
        };

        to_embed.push((vec_path.clone(), record.content_hash.clone(), embed_text));
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

    // Load children into a single vec (no HashMap)
    struct QueryChild {
        path: String,
        vector: Vec<f32>,
        first_line: String,
    }
    let mut children: Vec<QueryChild> = Vec::new();

    let mut vec_entries: Vec<_> = std::fs::read_dir(&sem_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "vec"))
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

        let first_line = record.summary.lines().next().unwrap_or("").trim().to_string();
        children.push(QueryChild { path: record.path, vector: vec_data.vector, first_line });
    }

    if children.is_empty() {
        return Ok(vec![]);
    }

    let query_vec = embed_query(query, model)?;

    let children_refs: Vec<(&str, &[f32])> = children
        .iter()
        .map(|c| (c.path.as_str(), c.vector.as_slice()))
        .collect();
    let ranked = cosine_rank(&query_vec, &children_refs);

    let child_lookup: std::collections::HashMap<&str, &QueryChild> = children
        .iter()
        .map(|c| (c.path.as_str(), c))
        .collect();

    let mut results = Vec::new();
    for (path, score) in ranked {
        if let Some(t) = threshold {
            if score < t {
                continue;
            }
        }
        let first_line = child_lookup
            .get(path.as_str())
            .map(|c| c.first_line.clone())
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
/// Adaptive beam selection: adjusts how many children to explore based on
/// child count and score distribution.
///
/// - Low fan-out (<=beam_width): take all children — nothing to prune
/// - Score gap: if there's a >0.05 drop between consecutive scores, cut there
///   (but always take at least beam_width, and at most 2*beam_width)
fn adaptive_beam(ranked: &[(String, f32)], beam_width: usize) -> Vec<(String, f32)> {
    let n = ranked.len();

    // Low fan-out: take everything
    if n <= beam_width {
        return ranked.to_vec();
    }

    // Always take at least beam_width
    let mut take = beam_width;
    let max_take = (2 * beam_width).min(n);

    // Extend past beam_width if no clear score gap
    for i in beam_width..max_take {
        let gap = ranked[i - 1].1 - ranked[i].1;
        if gap > 0.05 {
            break; // clear drop-off, stop here
        }
        take = i + 1;
    }

    ranked[..take].to_vec()
}

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
    let mut queue: std::collections::VecDeque<(std::path::PathBuf, String, usize)> =
        std::collections::VecDeque::from([(target.to_path_buf(), String::new(), 0)]);

    while let Some((dir_abs, dir_rel, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let level_start = Instant::now();
        let sem_dir = dir_abs.join(SEM_DIR);
        if !sem_dir.is_dir() {
            continue;
        }

        // Load child vectors and metadata into a single vec (no HashMap)
        struct ChildInfo {
            path: String,
            vector: Vec<f32>,
            first_line: String,
            is_dir: bool,
        }
        let mut children: Vec<ChildInfo> = Vec::new();

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

            let first_line = record.summary.lines().next().unwrap_or("").trim().to_string();
            children.push(ChildInfo {
                path: record.path,
                vector: vec_data.vector,
                first_line,
                is_dir: record.node_type == "directory",
            });
        }

        if children.is_empty() {
            continue;
        }

        let children_refs: Vec<(&str, &[f32])> = children
            .iter()
            .map(|c| (c.path.as_str(), c.vector.as_slice()))
            .collect();
        let ranked = cosine_rank(&query_vec, &children_refs);

        let selected = adaptive_beam(&ranked, beam_width);

        // Build a quick lookup for selected children
        let child_lookup: std::collections::HashMap<&str, &ChildInfo> = children
            .iter()
            .map(|c| (c.path.as_str(), c))
            .collect();

        let level_info = RouteLevel {
            dir: if dir_rel.is_empty() {
                ".".to_string()
            } else {
                dir_rel.clone()
            },
            selected: selected
                .iter()
                .map(|(path, score)| {
                    let first_line = child_lookup
                        .get(path.as_str())
                        .map(|c| c.first_line.clone())
                        .unwrap_or_default();
                    (path.clone(), *score, first_line)
                })
                .collect(),
            all_children: ranked.len(),
            elapsed_ms: level_start.elapsed().as_millis() as u64,
        };
        levels.push(level_info);

        // Queue directory children for descent
        for (child_path, _score) in &selected {
            if let Some(info) = child_lookup.get(child_path.as_str()) {
                if info.is_dir {
                    let child_abs = target.join(child_path);
                    if child_abs.is_dir() {
                        queue.push_back((child_abs, child_path.clone(), depth + 1));
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

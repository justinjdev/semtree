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

/// Policy for beam allocation across descent levels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum BeamPolicy {
    /// Fixed beam width at every level (current behavior)
    #[default]
    Uniform,
    /// Allocate beam proportional to alpha_l = B_l * m_l (water-filling)
    Waterfill,
}

/// A single level in the route descent.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RouteLevel {
    pub dir: String,
    pub selected: Vec<(String, f32, String)>, // (path, score, first_line)
    pub all_children: usize,
    pub elapsed_ms: u64,
    pub branching_factor: Option<usize>,
    pub ambiguity: Option<f32>,
    pub allocated_beam: Option<usize>,
}

// ---------------------------------------------------------------------------
// Native embedding via fastembed (ONNX Runtime)
// ---------------------------------------------------------------------------

static MODEL: Mutex<Option<TextEmbedding>> = Mutex::new(None);
/// The model name currently loaded (to detect model switches).
static MODEL_NAME: Mutex<Option<String>> = Mutex::new(None);

/// Resolve model string to fastembed enum.
fn resolve_model(model_name: &str) -> Result<EmbeddingModel> {
    match model_name {
        "BAAI/bge-small-en-v1.5" => Ok(EmbeddingModel::BGESmallENV15),
        "BAAI/bge-base-en-v1.5" => Ok(EmbeddingModel::BGEBaseENV15),
        "BAAI/bge-base-en-v1.5-q" => Ok(EmbeddingModel::BGEBaseENV15Q),
        "nomic-ai/nomic-embed-text-v1.5" => Ok(EmbeddingModel::NomicEmbedTextV15),
        "nomic-ai/nomic-embed-text-v1.5-q" => Ok(EmbeddingModel::NomicEmbedTextV15Q),
        other => bail!("Unsupported model: {other}. Supported: BAAI/bge-small-en-v1.5, BAAI/bge-base-en-v1.5[-q], nomic-ai/nomic-embed-text-v1.5[-q]"),
    }
}

/// Native dimensions for each model.
fn model_native_dims(model_name: &str) -> usize {
    match model_name {
        "BAAI/bge-small-en-v1.5" => 384,
        _ => 768, // bge-base, nomic v1.5 variants
    }
}

fn get_model(model_name: &str) -> Result<std::sync::MutexGuard<'static, Option<TextEmbedding>>> {
    let mut guard = MODEL.lock().map_err(|e| anyhow::anyhow!("model lock poisoned: {e}"))?;
    let mut name_guard = MODEL_NAME.lock().map_err(|e| anyhow::anyhow!("name lock poisoned: {e}"))?;

    let needs_load = match name_guard.as_deref() {
        Some(loaded) => loaded != model_name,
        None => true,
    };

    if needs_load {
        let model_enum = resolve_model(model_name)?;
        let options = InitOptions::new(model_enum).with_show_download_progress(true);
        let model = TextEmbedding::try_new(options).context("loading embedding model")?;
        *guard = Some(model);
        *name_guard = Some(model_name.to_string());
    }
    Ok(guard)
}

/// Truncate vectors to target_dims for Matryoshka-compatible models (nomic).
/// Re-normalizes after truncation.
fn maybe_truncate(vectors: Vec<Vec<f32>>, model: &str, target_dims: Option<usize>) -> Vec<Vec<f32>> {
    let target = match target_dims {
        Some(d) => d,
        None => return vectors,
    };
    let native = model_native_dims(model);
    if target >= native {
        return vectors;
    }
    vectors.into_iter().map(|v| {
        let truncated: Vec<f32> = v[..target].to_vec();
        let norm = dot(&truncated, &truncated).sqrt();
        if norm > 0.0 {
            truncated.into_iter().map(|x| x / norm).collect()
        } else {
            truncated
        }
    }).collect()
}

/// Embed a batch of document texts. Returns list of float vectors.
/// If target_dims is set and model supports Matryoshka, truncates to that size.
pub fn embed_texts(texts: &[String], model: &str) -> Result<Vec<Vec<f32>>> {
    embed_texts_with_dims(texts, model, None)
}

pub fn embed_texts_with_dims(texts: &[String], model: &str, target_dims: Option<usize>) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }
    let guard = get_model(model)?;
    let m = guard.as_ref().unwrap();
    let embeddings = m.embed(texts.to_vec(), None)?;
    Ok(maybe_truncate(embeddings, model, target_dims))
}

/// Embed a single query string. Returns a float vector.
pub fn embed_query(query: &str, model: &str) -> Result<Vec<f32>> {
    embed_query_with_dims(query, model, None)
}

pub fn embed_query_with_dims(query: &str, model: &str, target_dims: Option<usize>) -> Result<Vec<f32>> {
    let guard = get_model(model)?;
    let m = guard.as_ref().unwrap();
    let embeddings = m.embed(vec![format!("query: {query}")], None)?;
    let vecs = maybe_truncate(embeddings, model, target_dims);
    vecs.into_iter()
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
// Summary compaction for embedding
// ---------------------------------------------------------------------------

/// Compact a __dir__.md summary to fit within ~512 tokens for embedding.
///
/// Strategy:
/// 1. Keep the overview paragraph (before any ## heading)
/// 2. Keep the full ## Cross-Cutting Concerns section
/// 3. Compress ## Children to just "child-name: first-phrase" per line
/// 4. If still over budget, truncate children list
fn compact_dir_summary(full_summary: &str) -> String {
    let mut overview = String::new();
    let mut cross_cutting = String::new();
    let mut children_compact = Vec::new();

    enum Section { Overview, CrossCutting, Children, Other }
    let mut current = Section::Overview;

    for line in full_summary.lines() {
        if line.starts_with("## Cross-Cutting") {
            current = Section::CrossCutting;
            continue;
        } else if line.starts_with("## Children") {
            current = Section::Children;
            continue;
        } else if line.starts_with("## ") {
            current = Section::Other;
            continue;
        }

        match current {
            Section::Overview => {
                if !line.trim().is_empty() {
                    if !overview.is_empty() { overview.push(' '); }
                    overview.push_str(line.trim());
                }
            }
            Section::CrossCutting => {
                if !line.trim().is_empty() {
                    cross_cutting.push_str(line);
                    cross_cutting.push('\n');
                }
            }
            Section::Children => {
                // "- **child**: Full description..." -> "child: First sentence"
                if let Some(rest) = line.strip_prefix("- **") {
                    if let Some(name_end) = rest.find("**") {
                        let name = &rest[..name_end];
                        let desc = rest[name_end + 2..].trim_start_matches(':').trim();
                        // Take first sentence or first 80 chars
                        let short = desc
                            .split_once(". ")
                            .map(|(first, _)| first)
                            .unwrap_or(desc);
                        let short = if short.len() > 80 {
                            let mut end = 80;
                            while !short.is_char_boundary(end) { end -= 1; }
                            &short[..end]
                        } else { short };
                        children_compact.push(format!("{name}: {short}"));
                    }
                }
            }
            Section::Other => {}
        }
    }

    // Build compact text, estimating ~1.3 tokens per word
    let mut result = overview.clone();

    if !cross_cutting.is_empty() {
        result.push_str("\n\nCross-cutting: ");
        result.push_str(cross_cutting.trim());
    }

    if !children_compact.is_empty() {
        result.push_str("\n\nContains: ");
        // Add children until we approach the token budget
        let budget_chars = 1800; // ~460 tokens at 4 chars/token
        for child in &children_compact {
            if result.len() + child.len() + 2 > budget_chars {
                break;
            }
            result.push_str(child);
            result.push_str(". ");
        }
    }

    result
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

        // For directory siblings, build a compact embedding text from __dir__.md
        // that fits within the model's context window (~512 tokens).
        // Prioritizes: overview + cross-cutting concerns + compressed child list.
        let embed_text = if record.node_type == "directory" && !md_path.ends_with(records::DIR_RECORD) {
            let dir_record = target.join(&record.path).join(SEM_DIR).join(records::DIR_RECORD);
            if let Ok(Some(dir_rec)) = records::read_record(&dir_record) {
                compact_dir_summary(&dir_rec.summary)
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

    let total = to_embed.len();
    eprintln!("Embedding {total} records...");

    // Embed in chunks with progress
    let chunk_size = 256;
    let mut all_vectors: Vec<Vec<f32>> = Vec::with_capacity(total);

    for (chunk_idx, chunk) in to_embed.chunks(chunk_size).enumerate() {
        let texts: Vec<String> = chunk.iter().map(|(_, _, s)| s.clone()).collect();
        let vectors = embed_texts(&texts, model)?;
        let done = (chunk_idx + 1) * chunk_size;
        eprint!("\r  {}/{total} embedded...", done.min(total));
        all_vectors.extend(vectors);
    }
    eprintln!();

    for ((vec_path, content_hash, _), vector) in to_embed.iter().zip(all_vectors.iter()) {
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

/// Compute ambiguity m_l from cosine similarity scores.
/// High ambiguity (clustered scores) -> high m_l -> wider beam needed.
/// Low ambiguity (spread scores) -> low m_l -> narrow beam sufficient.
pub fn compute_ambiguity(scores: &[f32]) -> f32 {
    if scores.len() < 4 {
        return 0.5; // default for small fan-out
    }
    let mut sorted: Vec<f32> = scores.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let q25 = sorted[n / 4];
    let q75 = sorted[3 * n / 4];
    let iqr = q75 - q25;
    // High IQR = spread out = easy to distinguish = low ambiguity
    // Low IQR = clustered = hard to distinguish = high ambiguity
    (1.0 - iqr).clamp(0.1, 1.0)
}

/// Water-filling beam allocation: allocate beam width proportional to difficulty.
/// Returns (selected children, beam_used).
///
/// Harder levels (high alpha_l = B_l * m_l) get wider beams relative to a
/// baseline difficulty of 1.0. The per-level share is:
///   b_l = (remaining_budget / remaining_levels) * (alpha_l / alpha_ref)
/// clamped to [1, remaining_budget].
const ALPHA_REF: f32 = 3.0; // baseline: ~3 children at moderate ambiguity

fn waterfill_beam(
    ranked: &[(String, f32)],
    branching_factor: usize,
    ambiguity: f32,
    remaining_budget: usize,
    remaining_levels: usize,
) -> (Vec<(String, f32)>, usize) {
    let n = ranked.len();
    if n == 0 {
        return (vec![], 0);
    }

    // alpha_l = B_l * m_l (difficulty of this level)
    let alpha_l = branching_factor as f32 * ambiguity;

    // Base share: uniform split of remaining budget
    let base_share = remaining_budget as f32 / remaining_levels.max(1) as f32;

    // Scale by difficulty relative to baseline
    let scale = alpha_l / ALPHA_REF;
    let b_l = (base_share * scale).round().max(1.0) as usize;

    // Don't exceed remaining children or budget
    let take = b_l.min(n).min(remaining_budget);
    (ranked[..take].to_vec(), take)
}

pub fn route_directory(
    target: &Path,
    query: &str,
    model: &str,
    beam_width: usize,
    max_depth: usize,
) -> Result<Vec<RouteLevel>> {
    route_directory_with_policy(target, query, model, beam_width, max_depth, BeamPolicy::Uniform)
}

pub fn route_directory_with_policy(
    target: &Path,
    query: &str,
    model: &str,
    beam_width: usize,
    max_depth: usize,
    policy: BeamPolicy,
) -> Result<Vec<RouteLevel>> {
    let query_vec = embed_query(query, model)?;
    // Total budget for waterfill: beam_width * max_depth
    let mut remaining_budget = beam_width * max_depth;

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

        let remaining_levels = max_depth.saturating_sub(depth).max(1);
        let (selected, bf, amb, alloc_beam) = match policy {
            BeamPolicy::Uniform => {
                let sel = adaptive_beam(&ranked, beam_width);
                (sel, None, None, None)
            }
            BeamPolicy::Waterfill => {
                let bf = ranked.len();
                let scores: Vec<f32> = ranked.iter().map(|(_, s)| *s).collect();
                let amb = compute_ambiguity(&scores);
                let (sel, used) = waterfill_beam(&ranked, bf, amb, remaining_budget, remaining_levels);
                remaining_budget = remaining_budget.saturating_sub(used);
                (sel, Some(bf), Some(amb), Some(used))
            }
        };

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
            branching_factor: bf,
            ambiguity: amb,
            allocated_beam: alloc_beam,
        };
        levels.push(level_info);

        // Queue directory children for descent (skip self-references)
        for (child_path, _score) in &selected {
            if child_path == "." || child_path == &dir_rel {
                continue;
            }
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

/// Impact analysis: for each changed file, find the most similar files in the repo.
///
/// Returns a list of (changed_file, [(related_file, score, summary_first_line)]).
pub fn impact_analysis(
    target: &Path,
    changed_files: &[String],
    model: &str,
    top_k: usize,
) -> Result<Vec<(String, Vec<(String, f32, String)>)>> {
    // Load all file vectors + metadata
    struct FileEntry {
        path: String,
        vector: Vec<f32>,
        first_line: String,
    }
    let mut all_files: Vec<FileEntry> = Vec::new();

    for entry in walkdir::WalkDir::new(target)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.parent().map_or(false, |p| p.file_name().map_or(false, |n| n == SEM_DIR)) {
            continue;
        }
        if !path.extension().map_or(false, |e| e == "vec") {
            continue;
        }
        if path.file_stem().map_or(false, |s| s == "__dir__") {
            continue;
        }

        let vec_data = match vec_store::read_vec(path)? {
            Some(d) => d,
            None => continue,
        };

        let md_path = path.with_extension("md");
        let record = match records::read_record(&md_path)? {
            Some(r) => r,
            None => continue,
        };

        if record.node_type == "directory" {
            continue; // only compare files
        }

        let first_line = record.summary.lines().next().unwrap_or("").trim().to_string();
        all_files.push(FileEntry {
            path: record.path,
            vector: vec_data.vector,
            first_line,
        });
    }

    // For each changed file, find most similar other files
    let changed_set: std::collections::HashSet<&str> = changed_files.iter().map(|s| s.as_str()).collect();

    let mut results = Vec::new();
    for changed in changed_files {
        // Find this file's vector
        let source = match all_files.iter().find(|f| f.path == *changed) {
            Some(f) => f,
            None => {
                eprintln!("  WARN: no .vec for {changed}, skipping");
                continue;
            }
        };

        // Score against all other files
        let children_refs: Vec<(&str, &[f32])> = all_files
            .iter()
            .filter(|f| !changed_set.contains(f.path.as_str()))
            .map(|f| (f.path.as_str(), f.vector.as_slice()))
            .collect();

        let ranked = cosine_rank(&source.vector, &children_refs);

        let top: Vec<(String, f32, String)> = ranked
            .into_iter()
            .take(top_k)
            .map(|(path, score)| {
                let first_line = all_files
                    .iter()
                    .find(|f| f.path == path)
                    .map(|f| f.first_line.clone())
                    .unwrap_or_default();
                (path, score, first_line)
            })
            .collect();

        results.push((changed.clone(), top));
    }

    Ok(results)
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

    // --- compute_ambiguity tests (task 2.2) ---

    #[test]
    fn test_ambiguity_clustered_scores() {
        // All scores similar -> high ambiguity (close to 1.0)
        let scores = vec![0.80, 0.81, 0.79, 0.80, 0.82];
        let amb = compute_ambiguity(&scores);
        assert!(amb > 0.9, "clustered scores should have high ambiguity, got {amb}");
    }

    #[test]
    fn test_ambiguity_spread_scores() {
        // Scores spread out -> low ambiguity
        let scores = vec![0.1, 0.3, 0.5, 0.7, 0.9];
        let amb = compute_ambiguity(&scores);
        assert!(amb < 0.7, "spread scores should have low ambiguity, got {amb}");
    }

    #[test]
    fn test_ambiguity_fewer_than_4() {
        assert_eq!(compute_ambiguity(&[0.5, 0.6, 0.7]), 0.5);
        assert_eq!(compute_ambiguity(&[0.5]), 0.5);
        assert_eq!(compute_ambiguity(&[]), 0.5);
    }

    // --- waterfill_beam tests (task 3.3) ---

    #[test]
    fn test_waterfill_hard_level_wider_beam() {
        let ranked: Vec<(String, f32)> = (0..10).map(|i| (format!("c{i}"), 0.9 - i as f32 * 0.05)).collect();
        // Hard level: high branching * high ambiguity
        let (sel_hard, _) = waterfill_beam(&ranked, 10, 0.9, 20, 3);
        // Easy level: low branching * low ambiguity
        let (sel_easy, _) = waterfill_beam(&ranked, 3, 0.2, 20, 3);
        assert!(sel_hard.len() >= sel_easy.len(),
            "hard level should get wider beam: {} vs {}", sel_hard.len(), sel_easy.len());
    }

    #[test]
    fn test_waterfill_minimum_beam() {
        let ranked = vec![("a".to_string(), 0.9), ("b".to_string(), 0.5)];
        let (sel, used) = waterfill_beam(&ranked, 2, 0.1, 10, 5);
        assert!(sel.len() >= 1, "should always select at least 1");
        assert!(used >= 1);
    }

    #[test]
    fn test_waterfill_budget_not_exceeded() {
        let ranked: Vec<(String, f32)> = (0..20).map(|i| (format!("c{i}"), 0.9 - i as f32 * 0.01)).collect();
        let budget = 5;
        let (sel, used) = waterfill_beam(&ranked, 20, 1.0, budget, 1);
        assert!(used <= budget, "used {used} exceeds budget {budget}");
        assert!(sel.len() <= budget);
    }

    #[test]
    fn test_waterfill_single_level() {
        let ranked: Vec<(String, f32)> = (0..8).map(|i| (format!("c{i}"), 0.9 - i as f32 * 0.05)).collect();
        // With 1 remaining level, should use all remaining budget
        let (sel, _) = waterfill_beam(&ranked, 8, 0.5, 6, 1);
        assert_eq!(sel.len(), 6, "single level should use full budget (capped by children)");
    }

    #[test]
    fn test_waterfill_empty() {
        let (sel, used) = waterfill_beam(&[], 0, 0.5, 10, 3);
        assert!(sel.is_empty());
        assert_eq!(used, 0);
    }
}

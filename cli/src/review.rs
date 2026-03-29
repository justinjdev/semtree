use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use walkdir::WalkDir;

use crate::embedder;
use crate::records;
use crate::vec_store;

/// Semantic context loaded from .sem/ records for a changed file.
#[derive(Debug)]
struct FileContext {
    path: String,
    summary: String,
    first_line: String,
    parent_dir: String,
    module_summary: String,
    module_first_line: String,
}

/// Triage result for a changed file with fan-out and severity.
#[derive(Debug)]
struct FileTriage {
    path: String,
    severity: &'static str,
    fan_out: usize,
    first_line: String,
    related: Vec<(String, f32, String)>,
}

/// Entry point for the review command.
pub fn run(
    target: &Path,
    range: &str,
    _model: &str,
    top_k: usize,
    similarity_threshold: f32,
) -> Result<()> {
    // Task 2: get changed files from git diff
    let changed_files = get_changed_files(target, range)?;
    eprintln!("Changed files: {}", changed_files.len());

    // Task 3: load semantic context per file
    let contexts = load_file_contexts(target, &changed_files);
    eprintln!("Loaded context for {} file(s)", contexts.len());

    // Task 4: compute triage with fan-out and severity
    let triaged = compute_triage(target, &changed_files, top_k, similarity_threshold)?;

    for t in &triaged {
        println!(
            "[{}] {} (fan_out={})",
            t.severity, t.path, t.fan_out
        );
        if !t.first_line.is_empty() {
            println!("  {}", t.first_line);
        }
        for (rpath, score, fl) in &t.related {
            println!(
                "  {:.3}  {}  {}",
                score,
                rpath,
                &fl[..fl.len().min(70)]
            );
        }
    }

    Ok(())
}

/// Parse git diff to get list of changed files (repo-relative paths).
fn get_changed_files(target: &Path, range: &str) -> Result<Vec<String>> {
    let files = if !range.is_empty() {
        git_diff_name_only(target, &["diff", "--name-only", range])?
    } else {
        let mut f = git_diff_name_only(target, &["diff", "--name-only", "HEAD"])?;
        if f.is_empty() {
            f = git_diff_name_only(target, &["diff", "--name-only", "--cached"])?;
        }
        f
    };

    if files.is_empty() {
        bail!("No changed files found. Specify a range or have uncommitted changes.");
    }

    Ok(files)
}

fn git_diff_name_only(target: &Path, args: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(target)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect())
}

/// Load semantic context from .sem/ records for each changed file.
fn load_file_contexts(target: &Path, changed_files: &[String]) -> Vec<FileContext> {
    let mut contexts = Vec::new();

    for file in changed_files {
        let rec_path = records::record_path_for_file(target, file);
        let (summary, first_line) = match records::read_record(&rec_path) {
            Ok(Some(r)) => {
                let fl = r.summary.lines().next().unwrap_or("").trim().to_string();
                (r.summary, fl)
            }
            _ => (String::new(), String::new()),
        };

        // Parent directory context
        let parent_dir = Path::new(file)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let dir_rec_path = records::record_path_for_dir(target, &parent_dir);
        let (module_summary, module_first_line) = match records::read_record(&dir_rec_path) {
            Ok(Some(r)) => {
                let fl = r.summary.lines().next().unwrap_or("").trim().to_string();
                (r.summary, fl)
            }
            _ => (String::new(), String::new()),
        };

        contexts.push(FileContext {
            path: file.clone(),
            summary,
            first_line,
            parent_dir,
            module_summary,
            module_first_line,
        });
    }

    contexts
}

/// An entry from the vector store for similarity comparison.
struct VecEntry {
    path: String,
    vector: Vec<f32>,
    first_line: String,
}

/// Load all file vectors from .sem/*.vec files (skip __dir__ and directory records).
fn load_all_vectors(target: &Path) -> Result<Vec<VecEntry>> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(target)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path
            .parent()
            .map_or(false, |p| {
                p.file_name().map_or(false, |n| n == records::SEM_DIR)
            })
        {
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
            continue;
        }

        let first_line = record
            .summary
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        entries.push(VecEntry {
            path: record.path,
            vector: vec_data.vector,
            first_line,
        });
    }

    Ok(entries)
}

/// Compute triage for each changed file: fan-out, severity, and related files.
fn compute_triage(
    target: &Path,
    changed_files: &[String],
    top_k: usize,
    similarity_threshold: f32,
) -> Result<Vec<FileTriage>> {
    let all_vecs = load_all_vectors(target)?;
    let changed_set: HashSet<&str> = changed_files.iter().map(|s| s.as_str()).collect();

    let mut triaged = Vec::new();

    for changed in changed_files {
        // Find this file's vector
        let source = match all_vecs.iter().find(|e| e.path == *changed) {
            Some(e) => e,
            None => {
                eprintln!("  WARN: no .vec for {changed}, skipping triage");
                continue;
            }
        };

        // Compute similarity against all non-changed files
        let mut scored: Vec<(&str, f32, &str)> = Vec::new();
        let mut fan_out = 0usize;

        for other in &all_vecs {
            if changed_set.contains(other.path.as_str()) {
                continue;
            }
            let sim = embedder::cosine_similarity(&source.vector, &other.vector);
            if sim >= similarity_threshold {
                fan_out += 1;
            }
            scored.push((&other.path, sim, &other.first_line));
        }

        // Sort by similarity descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let related: Vec<(String, f32, String)> = scored
            .into_iter()
            .take(top_k)
            .map(|(p, s, fl)| (p.to_string(), s, fl.to_string()))
            .collect();

        let severity = if fan_out >= 10 {
            "HIGH"
        } else if fan_out >= 5 {
            "MEDIUM"
        } else {
            "LOW"
        };

        let first_line = source.first_line.clone();

        triaged.push(FileTriage {
            path: changed.clone(),
            severity,
            fan_out,
            first_line,
            related,
        });
    }

    // Sort by fan_out descending
    triaged.sort_by(|a, b| b.fan_out.cmp(&a.fan_out));

    Ok(triaged)
}

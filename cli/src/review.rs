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
    let changed_files = get_changed_files(target, range)?;
    eprintln!("Changed files: {}", changed_files.len());

    let contexts = load_file_contexts(target, &changed_files);
    eprintln!("Loaded context for {} file(s)", contexts.len());

    let triaged = compute_triage(target, &changed_files, top_k, similarity_threshold)?;

    let cc_warnings = find_cross_cutting_warnings(target, &changed_files, &contexts);
    let consider_also = find_consider_also(&triaged, &changed_files, similarity_threshold);
    render_markdown(&triaged, &contexts, &cc_warnings, &consider_also);
    eprintln!(
        "\nReview manifest: {} files triaged, {} cross-cutting warnings, {} suggestions",
        triaged.len(),
        cc_warnings.len(),
        consider_also.len()
    );

    Ok(())
}

/// Render the review manifest as markdown to stdout.
fn render_markdown(
    triaged: &[FileTriage],
    contexts: &[FileContext],
    cc_warnings: &[CrossCuttingWarning],
    consider_also: &[ConsiderAlso],
) {
    // Section 1: Triage table
    println!("# Review Manifest\n");
    println!("## Triage\n");
    println!("| File | Severity | Fan-out | Summary |");
    println!("|------|----------|---------|---------|");
    for t in triaged {
        let summary: String = if t.first_line.chars().count() > 60 {
            format!("{}...", t.first_line.chars().take(60).collect::<String>())
        } else {
            t.first_line.clone()
        };
        println!("| {} | {} | {} | {} |", t.path, t.severity, t.fan_out, summary);
    }

    // Section 2: Per-file context
    for t in triaged {
        let ctx = contexts.iter().find(|c| c.path == t.path);
        println!("\n---\n");
        println!("## {} [{}]\n", t.path, t.severity);

        let file_first_line = ctx.map(|c| c.first_line.as_str()).unwrap_or(&t.first_line);
        println!("**Summary:** {}\n", file_first_line);

        if let Some(c) = ctx {
            if !c.module_first_line.is_empty() {
                println!("**Module context ({}/):** {}\n", c.parent_dir, c.module_first_line);
            }
        }

        if !t.related.is_empty() {
            println!("**Related files to review:**");
            for (rpath, score, fl) in &t.related {
                println!("- {} ({:.2}) — {}", rpath, score, fl);
            }
        }
    }

    // Section 3: Cross-cutting warnings (only if there are any)
    if !cc_warnings.is_empty() || !consider_also.is_empty() {
        println!("\n---\n");
        println!("## Cross-Cutting Warnings\n");

        if !cc_warnings.is_empty() {
            println!("### High Confidence");
            println!("These files are explicitly documented as collaborators with changed files:");
            for w in cc_warnings {
                println!(
                    "- **{}** not in diff — {} (from {})",
                    w.collaborator, w.context, w.source_dir
                );
            }
            println!();
        }

        if !consider_also.is_empty() {
            println!("### Consider Also");
            println!("These files are highly similar to changed files but not in the diff:");
            for ca in consider_also {
                println!(
                    "- {} ({:.2} similar to {}) — {}",
                    ca.file, ca.score, ca.similar_to, ca.first_line
                );
            }
        }
    }
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

/// A cross-cutting warning: a file documented as a collaborator with a changed file.
#[derive(Debug)]
struct CrossCuttingWarning {
    collaborator: String,
    changed_file: String,
    context: String,
    source_dir: String,
}

/// A suggestion: a file highly similar to a changed file but not in the diff.
#[derive(Debug)]
struct ConsiderAlso {
    file: String,
    similar_to: String,
    score: f32,
    first_line: String,
}

/// Find cross-cutting warnings by scanning __dir__.md Cross-Cutting Concerns sections.
fn find_cross_cutting_warnings(
    target: &Path,
    changed_files: &[String],
    contexts: &[FileContext],
) -> Vec<CrossCuttingWarning> {
    let changed_set: HashSet<&str> = changed_files.iter().map(|s| s.as_str()).collect();

    // Collect basenames (lowercase, no extension) of changed files for matching
    let changed_basenames: HashSet<String> = changed_files
        .iter()
        .filter_map(|f| {
            Path::new(f)
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
        })
        .collect();

    // Unique parent directories from contexts
    let parent_dirs: HashSet<&str> = contexts.iter().map(|c| c.parent_dir.as_str()).collect();

    let mut warnings = Vec::new();

    for dir in &parent_dirs {
        let dir_rec_path = records::record_path_for_dir(target, dir);
        let summary = match records::read_record(&dir_rec_path) {
            Ok(Some(r)) => r.summary,
            _ => continue,
        };

        // Find the Cross-Cutting Concerns section
        let mut in_section = false;
        for line in summary.lines() {
            if line.starts_with("## Cross-Cutting Concerns") {
                in_section = true;
                continue;
            }
            if in_section && line.starts_with("## ") {
                break; // next section
            }
            if !in_section {
                continue;
            }

            let line_lower = line.to_lowercase();

            // Check if any changed file's basename is mentioned in this line
            for changed in changed_files {
                let basename = match Path::new(changed).file_stem() {
                    Some(s) => s.to_string_lossy().to_lowercase(),
                    None => continue,
                };
                if !line_lower.contains(&basename) {
                    continue;
                }

                // Look for other filenames in this line (word.ext patterns)
                for word in line.split_whitespace() {
                    let word_clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-');
                    if word_clean.len() <= 3 || !word_clean.contains('.') {
                        continue;
                    }
                    // Check it looks like a filename (has extension)
                    let parts: Vec<&str> = word_clean.rsplitn(2, '.').collect();
                    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                        continue;
                    }
                    let candidate_stem = parts[1].to_lowercase();
                    // Skip if this is a changed file's basename
                    if changed_basenames.contains(&candidate_stem) {
                        continue;
                    }
                    // Skip if this candidate is in the changed set (full path check)
                    if changed_set.iter().any(|cf| {
                        Path::new(cf)
                            .file_name()
                            .map_or(false, |n| n.to_string_lossy() == word_clean)
                    }) {
                        continue;
                    }

                    let source_dir = if dir.is_empty() {
                        ".".to_string()
                    } else {
                        dir.to_string()
                    };

                    warnings.push(CrossCuttingWarning {
                        collaborator: word_clean.to_string(),
                        changed_file: changed.clone(),
                        context: line.trim().to_string(),
                        source_dir: format!("{}/.sem/__dir__.md", source_dir),
                    });
                }
            }
        }
    }

    warnings
}

/// Find files similar to changed files that are not in the diff.
fn find_consider_also(
    triages: &[FileTriage],
    changed_files: &[String],
    similarity_threshold: f32,
) -> Vec<ConsiderAlso> {
    let changed_set: HashSet<&str> = changed_files.iter().map(|s| s.as_str()).collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut results = Vec::new();

    for triage in triages {
        for (path, score, first_line) in &triage.related {
            if *score < similarity_threshold {
                continue;
            }
            if changed_set.contains(path.as_str()) {
                continue;
            }
            if seen.contains(path) {
                continue;
            }
            seen.insert(path.clone());
            results.push(ConsiderAlso {
                file: path.clone(),
                similar_to: triage.path.clone(),
                score: *score,
                first_line: first_line.clone(),
            });
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results
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

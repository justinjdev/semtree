# semtree review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `semtree review` command that generates a structured review manifest (triage table, per-file context, cross-cutting warnings) from a git diff using existing .sem/ records and .vec embeddings.

**Architecture:** New `review.rs` module handles all review logic. Reuses `impact_analysis` pattern for loading vectors and computing similarity, `records::read_record` for .sem/ records. Renders markdown to stdout. New `Review` variant in `Commands` enum.

**Tech Stack:** Rust, clap (CLI), walkdir, existing embedder/records/vec_store modules.

---

### Task 1: Add Review command to CLI

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add Review variant to Commands enum**

In `cli/src/main.rs`, add after the `Impact` variant (around line 119):

```rust
    /// Generate a review manifest for code changes
    Review {
        /// Commit range (e.g., main..HEAD). Defaults to uncommitted changes.
        #[arg(default_value = "")]
        range: String,
        /// Repository root path
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Embedding model name
        #[arg(long, default_value = "BAAI/bge-small-en-v1.5")]
        model: String,
        /// Related files per changed file
        #[arg(long, default_value_t = 5)]
        top_k: usize,
        /// Cosine similarity threshold for cross-cutting warnings
        #[arg(long, default_value_t = 0.7)]
        similarity_threshold: f32,
    },
```

- [ ] **Step 2: Add mod declaration and stub dispatch**

Add `mod review;` to the module declarations at the top of main.rs.

Add the match arm in the main dispatch (after `Commands::Impact`):

```rust
        Commands::Review { range, path, model, top_k, similarity_threshold } => {
            let target = std::fs::canonicalize(&path)?;
            review::run(&target, &range, &model, top_k, similarity_threshold)?;
        }
```

- [ ] **Step 3: Create stub review.rs**

Create `cli/src/review.rs`:

```rust
use std::path::Path;
use anyhow::Result;

pub fn run(
    _target: &Path,
    _range: &str,
    _model: &str,
    _top_k: usize,
    _similarity_threshold: f32,
) -> Result<()> {
    eprintln!("semtree review: not yet implemented");
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cd cli && cargo build --release 2>&1 | grep error`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add cli/src/main.rs cli/src/review.rs
git commit -m "feat: add semtree review command stub"
```

---

### Task 2: Parse git diff to get changed files

**Files:**
- Modify: `cli/src/review.rs`

- [ ] **Step 1: Implement get_changed_files function**

```rust
use std::path::Path;
use std::process::Command;
use anyhow::{bail, Result};

/// Get changed file paths from a git diff range or working tree.
fn get_changed_files(target: &Path, range: &str) -> Result<Vec<String>> {
    if !range.is_empty() {
        // Explicit range: git diff --name-only base..head
        let output = Command::new("git")
            .args(["diff", "--name-only", range])
            .current_dir(target)
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect();
        if files.is_empty() {
            bail!("No changed files in range '{range}'");
        }
        return Ok(files);
    }

    // No range: try unstaged, then staged
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(target)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files: Vec<String> = stdout.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect();

    if files.is_empty() {
        let output = Command::new("git")
            .args(["diff", "--name-only", "--cached"])
            .current_dir(target)
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        files = stdout.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect();
    }

    if files.is_empty() {
        bail!("No changed files. Specify a range or have uncommitted changes.");
    }
    Ok(files)
}
```

- [ ] **Step 2: Wire into run()**

Update `run()` to call it and print the file list:

```rust
pub fn run(
    target: &Path,
    range: &str,
    _model: &str,
    _top_k: usize,
    _similarity_threshold: f32,
) -> Result<()> {
    let changed = get_changed_files(target, range)?;
    eprintln!("Review: {} changed file(s)", changed.len());
    for f in &changed {
        eprintln!("  {f}");
    }
    Ok(())
}
```

- [ ] **Step 3: Test manually**

Run against turborepo with a known range:
```bash
cli/target/release/semtree review HEAD~1..HEAD /path/to/turborepo
```
Expected: list of changed files from that commit

- [ ] **Step 4: Commit**

```bash
git add cli/src/review.rs
git commit -m "feat(review): parse git diff for changed files"
```

---

### Task 3: Load semantic context per file

**Files:**
- Modify: `cli/src/review.rs`

- [ ] **Step 1: Add FileContext struct and loader**

```rust
use crate::records::{self, SEM_DIR, DIR_RECORD};

struct FileContext {
    path: String,
    summary: String,
    first_line: String,
    parent_dir: String,
    module_summary: String,
    module_first_line: String,
}

fn load_file_context(target: &Path, file_path: &str) -> Option<FileContext> {
    // Load file's .sem/ record
    let rec_path = records::record_path_for_file(target, file_path);
    let record = records::read_record(&rec_path).ok()??;

    let first_line = record.summary.lines().next().unwrap_or("").trim().to_string();

    // Load parent directory's __dir__.md
    let parent = std::path::Path::new(file_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir_rec_path = records::record_path_for_dir(target, &parent);
    let (module_summary, module_first_line) = records::read_record(&dir_rec_path)
        .ok()
        .flatten()
        .map(|r| {
            let fl = r.summary.lines().next().unwrap_or("").trim().to_string();
            (r.summary, fl)
        })
        .unwrap_or_default();

    Some(FileContext {
        path: file_path.to_string(),
        summary: record.summary,
        first_line,
        parent_dir: parent,
        module_summary,
        module_first_line,
    })
}
```

- [ ] **Step 2: Load contexts in run()**

```rust
    let mut contexts: Vec<FileContext> = Vec::new();
    for f in &changed {
        if let Some(ctx) = load_file_context(target, f) {
            contexts.push(ctx);
        } else {
            eprintln!("  WARN: no .sem/ record for {f}");
        }
    }
    eprintln!("Loaded context for {}/{} files", contexts.len(), changed.len());
```

- [ ] **Step 3: Verify it compiles and runs**

```bash
cd cli && cargo build --release 2>&1 | grep error
```

- [ ] **Step 4: Commit**

```bash
git add cli/src/review.rs
git commit -m "feat(review): load semantic context from .sem/ records"
```

---

### Task 4: Compute fan-out and severity

**Files:**
- Modify: `cli/src/review.rs`

- [ ] **Step 1: Add fan-out computation using existing embedder infrastructure**

```rust
use crate::vec_store;
use crate::embedder;
use walkdir::WalkDir;

struct FileVector {
    path: String,
    vector: Vec<f32>,
    first_line: String,
}

/// Load all file vectors from the repo (same pattern as impact_analysis).
fn load_all_vectors(target: &Path) -> Result<Vec<FileVector>> {
    let mut all: Vec<FileVector> = Vec::new();
    for entry in WalkDir::new(target).into_iter().filter_map(|e| e.ok()) {
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
            continue;
        }
        let first_line = record.summary.lines().next().unwrap_or("").trim().to_string();
        all.push(FileVector {
            path: record.path,
            vector: vec_data.vector,
            first_line,
        });
    }
    Ok(all)
}

struct FileTriage {
    path: String,
    severity: &'static str,
    fan_out: usize,
    first_line: String,
    related: Vec<(String, f32, String)>, // (path, score, first_line)
}

fn compute_triage(
    changed: &[String],
    all_vectors: &[FileVector],
    top_k: usize,
    threshold: f32,
) -> Vec<FileTriage> {
    let changed_set: std::collections::HashSet<&str> =
        changed.iter().map(|s| s.as_str()).collect();

    let mut triages: Vec<FileTriage> = Vec::new();

    for changed_path in changed {
        let source = match all_vectors.iter().find(|f| f.path == *changed_path) {
            Some(f) => f,
            None => continue,
        };

        let mut fan_out = 0;
        let mut scored: Vec<(String, f32, String)> = Vec::new();

        for other in all_vectors {
            if changed_set.contains(other.path.as_str()) {
                continue;
            }
            let sim = embedder::cosine_similarity(&source.vector, &other.vector);
            if sim >= threshold {
                fan_out += 1;
            }
            scored.push((other.path.clone(), sim, other.first_line.clone()));
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let related: Vec<(String, f32, String)> = scored.into_iter().take(top_k).collect();

        let severity = match fan_out {
            n if n >= 10 => "HIGH",
            n if n >= 5 => "MEDIUM",
            _ => "LOW",
        };

        triages.push(FileTriage {
            path: changed_path.clone(),
            severity,
            fan_out,
            first_line: source.first_line.clone(),
            related,
        });
    }

    triages.sort_by(|a, b| b.fan_out.cmp(&a.fan_out));
    triages
}
```

- [ ] **Step 2: Wire into run()**

After loading contexts:

```rust
    eprintln!("Loading embeddings...");
    let all_vectors = load_all_vectors(target)?;
    eprintln!("Loaded {} file vectors", all_vectors.len());

    let triages = compute_triage(&changed, &all_vectors, top_k, similarity_threshold);
```

- [ ] **Step 3: Verify it compiles**

```bash
cd cli && cargo build --release 2>&1 | grep error
```

- [ ] **Step 4: Commit**

```bash
git add cli/src/review.rs
git commit -m "feat(review): compute fan-out severity and related files"
```

---

### Task 5: Parse cross-cutting concerns from __dir__.md

**Files:**
- Modify: `cli/src/review.rs`

- [ ] **Step 1: Add cross-cutting parser**

```rust
struct CrossCuttingWarning {
    collaborator: String,
    changed_file: String,
    context: String,   // the line from the cross-cutting section
    source_dir: String, // which __dir__.md it came from
}

/// Parse ## Cross-Cutting Concerns from a directory summary and find
/// collaborators that should have changed but didn't.
fn find_cross_cutting_warnings(
    target: &Path,
    changed: &[String],
    contexts: &[FileContext],
) -> Vec<CrossCuttingWarning> {
    let changed_set: std::collections::HashSet<&str> =
        changed.iter().map(|s| s.as_str()).collect();
    // Collect basenames of changed files for fuzzy matching
    let changed_basenames: std::collections::HashMap<&str, &str> = changed.iter()
        .filter_map(|p| {
            std::path::Path::new(p.as_str())
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| (n, p.as_str()))
        })
        .collect();

    let mut warnings: Vec<CrossCuttingWarning> = Vec::new();

    // Collect unique parent directories from changed files
    let parent_dirs: std::collections::HashSet<String> = contexts.iter()
        .map(|c| c.parent_dir.clone())
        .collect();

    for dir in &parent_dirs {
        let dir_rec_path = records::record_path_for_dir(target, dir);
        let record = match records::read_record(&dir_rec_path).ok().flatten() {
            Some(r) => r,
            None => continue,
        };

        // Extract cross-cutting section
        let summary = &record.summary;
        let cc_start = match summary.find("## Cross-Cutting Concerns") {
            Some(i) => i,
            None => continue,
        };
        let cc_section = &summary[cc_start..];
        let cc_end = cc_section[1..].find("\n## ").map(|i| i + 1).unwrap_or(cc_section.len());
        let cc_text = &cc_section[..cc_end];

        // For each line, check if it mentions a changed file AND an unchanged file
        for line in cc_text.lines() {
            let line_lower = line.to_lowercase();
            // Check if any changed file basename appears in this line
            let mut mentions_changed: Option<&str> = None;
            for (basename, full_path) in &changed_basenames {
                let basename_no_ext = basename.rsplit('.').last().unwrap_or(basename);
                if line_lower.contains(&basename_no_ext.to_lowercase()) {
                    mentions_changed = Some(full_path);
                    break;
                }
            }

            if let Some(changed_file) = mentions_changed {
                // Look for other file names in this line that are NOT in the changed set
                // Match patterns like **filename.ext** or `filename.ext`
                for word in line.split(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-') {
                    if word.contains('.') && word.len() > 3 {
                        // Looks like a filename
                        if !changed_basenames.contains_key(word) {
                            // This file is mentioned but not changed — potential warning
                            warnings.push(CrossCuttingWarning {
                                collaborator: word.to_string(),
                                changed_file: changed_file.to_string(),
                                context: line.trim().to_string(),
                                source_dir: dir.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    warnings
}
```

- [ ] **Step 2: Add embedding-based "consider also" warnings**

```rust
struct ConsiderAlso {
    file: String,
    similar_to: String,
    score: f32,
    first_line: String,
}

fn find_consider_also(
    triages: &[FileTriage],
    changed: &[String],
    threshold: f32,
) -> Vec<ConsiderAlso> {
    let changed_set: std::collections::HashSet<&str> =
        changed.iter().map(|s| s.as_str()).collect();

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut suggestions: Vec<ConsiderAlso> = Vec::new();

    for triage in triages {
        for (related_path, score, first_line) in &triage.related {
            if *score >= threshold
                && !changed_set.contains(related_path.as_str())
                && !seen.contains(related_path)
            {
                seen.insert(related_path.clone());
                suggestions.push(ConsiderAlso {
                    file: related_path.clone(),
                    similar_to: triage.path.clone(),
                    score: *score,
                    first_line: first_line.clone(),
                });
            }
        }
    }

    suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    suggestions
}
```

- [ ] **Step 3: Wire into run()**

```rust
    let cc_warnings = find_cross_cutting_warnings(target, &changed, &contexts);
    let consider_also = find_consider_also(&triages, &changed, similarity_threshold);
```

- [ ] **Step 4: Verify it compiles**

```bash
cd cli && cargo build --release 2>&1 | grep error
```

- [ ] **Step 5: Commit**

```bash
git add cli/src/review.rs
git commit -m "feat(review): cross-cutting warnings from __dir__.md and embeddings"
```

---

### Task 6: Render markdown output

**Files:**
- Modify: `cli/src/review.rs`

- [ ] **Step 1: Add render function**

```rust
fn render_markdown(
    triages: &[FileTriage],
    contexts: &[FileContext],
    cc_warnings: &[CrossCuttingWarning],
    consider_also: &[ConsiderAlso],
) {
    // Section 1: Triage
    println!("# Review Manifest\n");
    println!("## Triage\n");
    println!("| File | Severity | Fan-out | Summary |");
    println!("|------|----------|---------|---------|");
    for t in triages {
        let summary = if t.first_line.len() > 60 {
            format!("{}...", &t.first_line[..57])
        } else {
            t.first_line.clone()
        };
        println!("| {} | {} | {} | {} |", t.path, t.severity, t.fan_out, summary);
    }

    // Section 2: Per-file context
    println!("\n---\n");
    for t in triages {
        let ctx = contexts.iter().find(|c| c.path == t.path);
        println!("## {} [{}]\n", t.path, t.severity);

        if let Some(c) = ctx {
            println!("**Summary:** {}\n", c.first_line);
            if !c.module_first_line.is_empty() {
                println!("**Module context ({}):** {}\n", c.parent_dir, c.module_first_line);
            }
        }

        if !t.related.is_empty() {
            println!("**Related files to review:**");
            for (path, score, first_line) in &t.related {
                let fl = if first_line.len() > 60 {
                    format!("{}...", &first_line[..57])
                } else {
                    first_line.clone()
                };
                println!("- {} ({:.2}) - {}", path, score, fl);
            }
            println!();
        }
    }

    // Section 3: Cross-cutting warnings
    if cc_warnings.is_empty() && consider_also.is_empty() {
        return;
    }

    println!("---\n");
    println!("## Cross-Cutting Warnings\n");

    if !cc_warnings.is_empty() {
        println!("### High Confidence\n");
        println!("These files are explicitly documented as collaborators with changed files:\n");
        for w in cc_warnings {
            println!("- **{}** not in diff - {} (from {}/.sem/__dir__.md)",
                w.collaborator, w.context, w.source_dir);
        }
        println!();
    }

    if !consider_also.is_empty() {
        println!("### Consider Also\n");
        println!("These files are highly similar to changed files but not in the diff:\n");
        for s in consider_also {
            println!("- {} ({:.2} similar to {}) - {}",
                s.file, s.score, s.similar_to, s.first_line);
        }
    }
}
```

- [ ] **Step 2: Wire into run() — complete the function**

Replace the full `run()` body:

```rust
pub fn run(
    target: &Path,
    range: &str,
    _model: &str,
    top_k: usize,
    similarity_threshold: f32,
) -> Result<()> {
    let changed = get_changed_files(target, range)?;
    eprintln!("Review: {} changed file(s)", changed.len());

    let mut contexts: Vec<FileContext> = Vec::new();
    for f in &changed {
        if let Some(ctx) = load_file_context(target, f) {
            contexts.push(ctx);
        } else {
            eprintln!("  WARN: no .sem/ record for {f}");
        }
    }

    eprintln!("Loading embeddings...");
    let all_vectors = load_all_vectors(target)?;
    eprintln!("Loaded {} file vectors", all_vectors.len());

    let triages = compute_triage(&changed, &all_vectors, top_k, similarity_threshold);
    let cc_warnings = find_cross_cutting_warnings(target, &changed, &contexts);
    let consider_also = find_consider_also(&triages, &changed, similarity_threshold);

    render_markdown(&triages, &contexts, &cc_warnings, &consider_also);

    eprintln!(
        "\nReview manifest: {} files triaged, {} cross-cutting warnings, {} suggestions",
        triages.len(), cc_warnings.len(), consider_also.len()
    );
    Ok(())
}
```

- [ ] **Step 3: Build and test end-to-end**

```bash
cd cli && cargo build --release 2>&1 | grep error
```

Then test against turborepo:
```bash
cli/target/release/semtree review HEAD~1..HEAD /path/to/turborepo
```

Expected: three-section markdown manifest printed to stdout.

- [ ] **Step 4: Commit**

```bash
git add cli/src/review.rs
git commit -m "feat(review): render three-section markdown manifest"
```

---

### Task 7: Integration test

**Files:**
- Modify: `cli/src/review.rs`

- [ ] **Step 1: Run full end-to-end test on turborepo**

```bash
# Test with explicit range
cli/target/release/semtree review HEAD~3..HEAD /Users/justin/git/turborepo > /tmp/review-test.md
cat /tmp/review-test.md

# Test with no args (needs uncommitted changes)
# Make a trivial change first:
echo "// test" >> /Users/justin/git/turborepo/crates/turborepo-engine/src/lib.rs
cli/target/release/semtree review /Users/justin/git/turborepo > /tmp/review-test2.md
cat /tmp/review-test2.md
git -C /Users/justin/git/turborepo checkout -- crates/turborepo-engine/src/lib.rs
```

Verify:
- Triage table has correct columns and severity buckets
- Per-file sections show summary, module context, related files
- Cross-cutting warnings appear (if any match)
- Consider-also suggestions appear for high-similarity files

- [ ] **Step 2: Final commit and push**

```bash
git add cli/src/review.rs cli/src/main.rs
git commit -m "feat: semtree review — review manifest generator

Generates structured review manifest from git diff:
- Triage table with severity from embedding fan-out
- Per-file semantic context from .sem/ records
- Cross-cutting warnings from __dir__.md + embedding similarity
No LLM calls — pure embedding math + record reading."
```

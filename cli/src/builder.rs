//! Build pipeline: walk -> hash -> check freshness -> summarize -> write records.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use crate::hasher;
use crate::records;
use crate::summarizer::{self, SummarizerFn};
use crate::walker;

/// Configuration for the build pipeline.
pub struct BuildConfig {
    pub target_path: PathBuf,
    pub model: String,
    pub max_tokens: usize,
    pub force: bool,
    pub exclude: Vec<String>,
    pub embed: bool,
    pub embed_model: String,
}

/// Statistics from a build run.
#[derive(Debug, Default)]
pub struct BuildStats {
    pub summarized: usize,
    pub skipped: usize,
    pub errored: usize,
}

/// Run the full SRT build pipeline using the auto-detected summarizer.
pub fn build(config: &BuildConfig) -> Result<BuildStats> {
    let summarizer = summarizer::create_summarizer(&config.model);
    build_with_summarizer(config, summarizer.as_ref())
}

/// Run the build pipeline with an injectable summarizer (for testing).
pub fn build_with_summarizer(
    config: &BuildConfig,
    summarizer: &dyn SummarizerFn,
) -> Result<BuildStats> {
    let nodes = walker::walk(&config.target_path, &config.exclude)?;
    let total = nodes.len();

    let mut node_hashes: HashMap<String, String> = HashMap::new();
    let mut node_summaries: HashMap<String, String> = HashMap::new();
    let mut stats = BuildStats::default();

    for (i, node) in nodes.iter().enumerate() {
        let idx = i + 1;
        let rel = &node.repo_relative_path;
        let label = if rel.is_empty() { "(root)" } else { rel.as_str() };

        if node.is_directory {
            // Compute directory hash from children
            let child_pairs: Vec<(&str, &str)> = node
                .children
                .iter()
                .filter_map(|c| {
                    node_hashes
                        .get(c.as_str())
                        .map(|h| (c.as_str(), h.as_str()))
                })
                .collect();
            let content_hash = hasher::hash_directory(&child_pairs);
            node_hashes.insert(rel.clone(), content_hash.clone());

            let rec_path = records::record_path_for_dir(&config.target_path, rel);
            let existing = records::read_record(&rec_path)?;

            if !config.force {
                if let Some(ref record) = existing {
                    if record.content_hash == content_hash {
                        node_summaries.insert(rel.clone(), record.summary.clone());
                        // Ensure sibling record exists at parent level
                        let sibling_path =
                            records::record_path_for_dir_sibling(&config.target_path, rel);
                        if sibling_path != rec_path {
                            let sibling_existing = records::read_record(&sibling_path)?;
                            let needs_sibling = match &sibling_existing {
                                Some(sr) => sr.content_hash != content_hash,
                                None => true,
                            };
                            if needs_sibling {
                                records::write_record(
                                    &sibling_path,
                                    rel,
                                    "directory",
                                    &content_hash,
                                    &record.summary,
                                )?;
                            }
                        }
                        stats.skipped += 1;
                        eprintln!("[{idx}/{total}] skip {label} (up-to-date)");
                        continue;
                    }
                }
            }

            // Build directory prompt from child summaries
            let child_summary_pairs: Vec<(&str, &str)> = node
                .children
                .iter()
                .map(|c| {
                    let summary = node_summaries
                        .get(c.as_str())
                        .map(|s| s.as_str())
                        .unwrap_or(summarizer::OVERSIZED_PLACEHOLDER);
                    (c.as_str(), summary)
                })
                .collect();
            let prompt = summarizer::build_dir_prompt(rel, &child_summary_pairs);

            match summarizer.call(&prompt) {
                Ok(summary) => {
                    let write_path = if rel.is_empty() { "." } else { rel.as_str() };
                    records::write_record(
                        &rec_path,
                        write_path,
                        "directory",
                        &content_hash,
                        &summary,
                    )?;
                    // Also write sibling record at parent level for embedding/routing
                    let sibling_path =
                        records::record_path_for_dir_sibling(&config.target_path, rel);
                    if sibling_path != rec_path {
                        records::write_record(
                            &sibling_path,
                            rel,
                            "directory",
                            &content_hash,
                            &summary,
                        )?;
                    }
                    node_summaries.insert(rel.clone(), summary);
                    stats.summarized += 1;
                    eprintln!("[{idx}/{total}] summarized {label}");
                }
                Err(e) => {
                    eprintln!("[{idx}/{total}] ERROR {label}: {e}");
                    stats.errored += 1;
                    node_summaries.insert(rel.clone(), String::new());
                }
            }
        } else {
            // File node
            let content_hash = hasher::hash_file(&node.absolute_path)?;
            node_hashes.insert(rel.clone(), content_hash.clone());

            let rec_path = records::record_path_for_file(&config.target_path, rel);
            let existing = records::read_record(&rec_path)?;

            if !config.force {
                if let Some(ref record) = existing {
                    if record.content_hash == content_hash {
                        node_summaries.insert(rel.clone(), record.summary.clone());
                        stats.skipped += 1;
                        eprintln!("[{idx}/{total}] skip {label} (up-to-date)");
                        continue;
                    }
                }
            }

            // Check oversized
            let file_size = std::fs::metadata(&node.absolute_path)?.len();
            if summarizer::is_oversized(file_size, config.max_tokens) {
                records::write_record(
                    &rec_path,
                    rel,
                    "file",
                    &content_hash,
                    summarizer::OVERSIZED_PLACEHOLDER,
                )?;
                node_summaries.insert(rel.clone(), summarizer::OVERSIZED_PLACEHOLDER.to_string());
                stats.summarized += 1;
                eprintln!("[{idx}/{total}] oversized {label}");
                continue;
            }

            let file_content = std::fs::read_to_string(&node.absolute_path)
                .unwrap_or_else(|_| String::from_utf8_lossy(&std::fs::read(&node.absolute_path).unwrap_or_default()).to_string());
            let prompt = summarizer::build_file_prompt(rel, &file_content);

            match summarizer.call(&prompt) {
                Ok(summary) => {
                    records::write_record(&rec_path, rel, "file", &content_hash, &summary)?;
                    node_summaries.insert(rel.clone(), summary);
                    stats.summarized += 1;
                    eprintln!("[{idx}/{total}] summarized {label}");
                }
                Err(e) => {
                    eprintln!("[{idx}/{total}] ERROR {label}: {e}");
                    stats.errored += 1;
                    node_summaries.insert(rel.clone(), String::new());
                }
            }
        }
    }

    eprintln!(
        "\nDone: {} summarized, {} skipped, {} errored",
        stats.summarized, stats.skipped, stats.errored
    );

    Ok(stats)
}

/// Build SRT using the Anthropic Batch API for file summaries (50% cost savings).
///
/// Files are batched into one API call. Directory summaries are done sequentially
/// after all file summaries are available (they depend on child summaries).
pub fn build_batch(config: &BuildConfig) -> Result<BuildStats> {
    let nodes = walker::walk(&config.target_path, &config.exclude)?;
    let total = nodes.len();

    let mut node_hashes: HashMap<String, String> = HashMap::new();
    let mut node_summaries: HashMap<String, String> = HashMap::new();
    let mut stats = BuildStats::default();

    // --- Phase 1: Walk, hash, identify stale files ---
    // (custom_id, prompt, rel_path, content_hash, rec_path)
    let mut batch_items: Vec<(String, String, String, String, PathBuf)> = Vec::new();
    // Directory nodes to process after files (in order)
    let mut dir_nodes: Vec<(usize, usize)> = Vec::new(); // (index into nodes, idx for display)

    for (i, node) in nodes.iter().enumerate() {
        let idx = i + 1;
        let rel = &node.repo_relative_path;

        if node.is_directory {
            // Compute hash, check freshness, defer summarization to phase 2
            let child_pairs: Vec<(&str, &str)> = node
                .children
                .iter()
                .filter_map(|c| {
                    node_hashes
                        .get(c.as_str())
                        .map(|h| (c.as_str(), h.as_str()))
                })
                .collect();
            let content_hash = hasher::hash_directory(&child_pairs);
            node_hashes.insert(rel.clone(), content_hash.clone());

            let rec_path = records::record_path_for_dir(&config.target_path, rel);
            let existing = records::read_record(&rec_path)?;

            if !config.force {
                if let Some(ref record) = existing {
                    if record.content_hash == content_hash {
                        node_summaries.insert(rel.clone(), record.summary.clone());
                        let sibling_path =
                            records::record_path_for_dir_sibling(&config.target_path, rel);
                        if sibling_path != rec_path {
                            let sibling_existing = records::read_record(&sibling_path)?;
                            let needs_sibling = match &sibling_existing {
                                Some(sr) => sr.content_hash != content_hash,
                                None => true,
                            };
                            if needs_sibling {
                                records::write_record(
                                    &sibling_path, rel, "directory", &content_hash, &record.summary,
                                )?;
                            }
                        }
                        stats.skipped += 1;
                        eprintln!("[{idx}/{total}] skip {} (up-to-date)", if rel.is_empty() { "(root)" } else { rel.as_str() });
                        continue;
                    }
                }
            }
            dir_nodes.push((i, idx));
        } else {
            // File node
            let content_hash = hasher::hash_file(&node.absolute_path)?;
            node_hashes.insert(rel.clone(), content_hash.clone());

            let rec_path = records::record_path_for_file(&config.target_path, rel);
            let existing = records::read_record(&rec_path)?;

            if !config.force {
                if let Some(ref record) = existing {
                    if record.content_hash == content_hash {
                        node_summaries.insert(rel.clone(), record.summary.clone());
                        stats.skipped += 1;
                        eprintln!("[{idx}/{total}] skip {rel} (up-to-date)");
                        continue;
                    }
                }
            }

            let file_size = std::fs::metadata(&node.absolute_path)?.len();
            if summarizer::is_oversized(file_size, config.max_tokens) {
                records::write_record(&rec_path, rel, "file", &content_hash, summarizer::OVERSIZED_PLACEHOLDER)?;
                node_summaries.insert(rel.clone(), summarizer::OVERSIZED_PLACEHOLDER.to_string());
                stats.summarized += 1;
                eprintln!("[{idx}/{total}] oversized {rel}");
                continue;
            }

            let file_content = std::fs::read_to_string(&node.absolute_path)
                .unwrap_or_else(|_| String::from_utf8_lossy(&std::fs::read(&node.absolute_path).unwrap_or_default()).to_string());
            let prompt = summarizer::build_file_prompt(rel, &file_content);
            let custom_id = format!("f{:0>6}", batch_items.len());
            batch_items.push((custom_id, prompt, rel.clone(), content_hash, rec_path));
        }
    }

    // --- Phase 2: Submit file batch ---
    if !batch_items.is_empty() {
        eprintln!("\nSubmitting batch of {} file summaries...", batch_items.len());
        let prompts: Vec<(String, String)> = batch_items
            .iter()
            .map(|(id, prompt, _, _, _)| (id.clone(), prompt.clone()))
            .collect();

        let batch_result = summarizer::submit_batch(&prompts, &config.model)?;
        eprintln!("Batch submitted: {}", batch_result.batch_id);

        let results = summarizer::poll_batch(&batch_result.batch_id)?;
        eprintln!("\nBatch complete: {} results", results.len());

        for (custom_id, prompt, rel, content_hash, rec_path) in &batch_items {
            if let Some(summary) = results.get(custom_id) {
                if !summary.is_empty() {
                    records::write_record(rec_path, rel, "file", content_hash, summary)?;
                    node_summaries.insert(rel.clone(), summary.clone());
                    stats.summarized += 1;
                } else {
                    eprintln!("  WARN: empty summary for {rel}");
                    stats.errored += 1;
                    node_summaries.insert(rel.clone(), String::new());
                }
            } else {
                eprintln!("  WARN: no result for {rel}");
                stats.errored += 1;
                node_summaries.insert(rel.clone(), String::new());
            }
        }
    }

    // --- Phase 3: Directory summaries batched by depth level ---
    // Directories at the same depth are independent (children already summarized).
    // Group by depth, batch each level via the Batch API.
    if !dir_nodes.is_empty() {
        eprintln!("\nSummarizing {} directories by depth level...", dir_nodes.len());

        // Group by depth (number of '/' in path)
        let mut depth_groups: std::collections::BTreeMap<usize, Vec<(usize, usize)>> =
            std::collections::BTreeMap::new();
        for &(node_idx, display_idx) in &dir_nodes {
            let rel = &nodes[node_idx].repo_relative_path;
            let depth = if rel.is_empty() { 0 } else { rel.matches('/').count() + 1 };
            depth_groups.entry(depth).or_default().push((node_idx, display_idx));
        }

        // Process deepest first (BTreeMap is sorted, reverse iterate)
        let depth_levels: Vec<_> = depth_groups.into_iter().rev().collect();
        for (depth, group) in &depth_levels {
            eprintln!("  Depth {depth}: {} directories", group.len());

            // Build prompts for this level
            let mut level_items: Vec<(String, String, String, String)> = Vec::new(); // (custom_id, prompt, rel, content_hash)
            for &(node_idx, _display_idx) in group {
                let node = &nodes[node_idx];
                let rel = &node.repo_relative_path;
                let content_hash = node_hashes.get(rel).cloned().unwrap_or_default();

                let child_summary_pairs: Vec<(&str, &str)> = node
                    .children
                    .iter()
                    .map(|c| {
                        let summary = node_summaries
                            .get(c.as_str())
                            .map(|s| s.as_str())
                            .unwrap_or(summarizer::OVERSIZED_PLACEHOLDER);
                        (c.as_str(), summary)
                    })
                    .collect();
                let prompt = summarizer::build_dir_prompt(rel, &child_summary_pairs);
                let custom_id = format!("d{:0>6}", level_items.len());
                level_items.push((custom_id, prompt, rel.clone(), content_hash));
            }

            // Submit batch for this depth level
            let prompts: Vec<(String, String)> = level_items
                .iter()
                .map(|(id, prompt, _, _)| (id.clone(), prompt.clone()))
                .collect();

            let batch_result = summarizer::submit_batch(&prompts, &config.model)?;
            eprintln!("  Batch submitted: {} ({} dirs)", batch_result.batch_id, prompts.len());

            let results = summarizer::poll_batch(&batch_result.batch_id)?;
            eprintln!("\n  Batch complete: {} results", results.len());

            // Write records and update summaries for next level
            for (custom_id, _prompt, rel, content_hash) in &level_items {
                let label = if rel.is_empty() { "(root)" } else { rel.as_str() };
                if let Some(summary) = results.get(custom_id) {
                    if !summary.is_empty() {
                        let write_path = if rel.is_empty() { "." } else { rel.as_str() };
                        let rec_path = records::record_path_for_dir(&config.target_path, rel);
                        records::write_record(&rec_path, write_path, "directory", content_hash, summary)?;
                        let sibling_path = records::record_path_for_dir_sibling(&config.target_path, rel);
                        if sibling_path != rec_path {
                            records::write_record(&sibling_path, rel, "directory", content_hash, summary)?;
                        }
                        node_summaries.insert(rel.clone(), summary.clone());
                        stats.summarized += 1;
                    } else {
                        eprintln!("  WARN: empty summary for {label}");
                        stats.errored += 1;
                        node_summaries.insert(rel.clone(), String::new());
                    }
                } else {
                    eprintln!("  WARN: no result for {label}");
                    stats.errored += 1;
                    node_summaries.insert(rel.clone(), String::new());
                }
            }
        }
    }

    eprintln!(
        "\nDone: {} summarized, {} skipped, {} errored",
        stats.summarized, stats.skipped, stats.errored
    );

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records;
    use crate::summarizer::SummarizerFn;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Mock summarizer that returns a deterministic summary.
    struct MockSummarizer;

    impl SummarizerFn for MockSummarizer {
        fn call(&self, _prompt: &str) -> Result<String> {
            Ok("Mock summary of the content.".to_string())
        }
    }

    /// Mock summarizer that counts calls.
    struct CountingSummarizer {
        count: AtomicUsize,
    }

    impl CountingSummarizer {
        fn new() -> Self {
            Self {
                count: AtomicUsize::new(0),
            }
        }
        fn call_count(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }

    impl SummarizerFn for CountingSummarizer {
        fn call(&self, _prompt: &str) -> Result<String> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok("Counted summary.".to_string())
        }
    }

    /// Mock summarizer that fails on specific calls.
    struct FailingSummarizer;

    impl SummarizerFn for FailingSummarizer {
        fn call(&self, _prompt: &str) -> Result<String> {
            anyhow::bail!("mock summarizer error")
        }
    }

    fn make_config(path: PathBuf) -> BuildConfig {
        BuildConfig {
            target_path: path,
            model: "test-model".to_string(),
            max_tokens: 100_000,
            force: false,
            exclude: vec![],
            embed: false,
            embed_model: String::new(),
        }
    }

    /// Set up a minimal non-git directory for testing.
    /// Creates files outside any git repo by ensuring no .git exists.
    fn setup_test_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "# Test Project").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "pub mod utils;").unwrap();

        tmp
    }

    #[test]
    fn test_full_pipeline_flow() {
        let tmp = setup_test_repo();
        let config = make_config(tmp.path().to_path_buf());

        let stats = build_with_summarizer(&config, &MockSummarizer).unwrap();

        // Should have summarized all files and directories
        assert!(stats.summarized > 0);
        assert_eq!(stats.errored, 0);

        // Verify records were written for files
        let rec = records::record_path_for_file(tmp.path(), "README.md");
        let record = records::read_record(&rec).unwrap().unwrap();
        assert_eq!(record.node_type, "file");
        assert_eq!(record.summary, "Mock summary of the content.");

        // Verify directory record
        let dir_rec = records::record_path_for_dir(tmp.path(), "src");
        let dir_record = records::read_record(&dir_rec).unwrap().unwrap();
        assert_eq!(dir_record.node_type, "directory");

        // Verify root directory record
        let root_rec = records::record_path_for_dir(tmp.path(), "");
        let root_record = records::read_record(&root_rec).unwrap().unwrap();
        assert_eq!(root_record.node_type, "directory");
    }

    #[test]
    fn test_incremental_skip() {
        let tmp = setup_test_repo();
        let config = make_config(tmp.path().to_path_buf());

        // First build: everything gets summarized
        let summarizer = CountingSummarizer::new();
        let stats1 = build_with_summarizer(&config, &summarizer).unwrap();
        let first_count = summarizer.call_count();
        assert!(first_count > 0);
        assert_eq!(stats1.skipped, 0);

        // Second build with same content: everything should be skipped
        let summarizer2 = CountingSummarizer::new();
        let stats2 = build_with_summarizer(&config, &summarizer2).unwrap();
        assert_eq!(summarizer2.call_count(), 0);
        assert_eq!(stats2.summarized, 0);
        assert!(stats2.skipped > 0);
        assert_eq!(stats2.skipped, first_count); // same number of nodes
    }

    #[test]
    fn test_force_rebuild() {
        let tmp = setup_test_repo();
        let mut config = make_config(tmp.path().to_path_buf());

        // First build
        build_with_summarizer(&config, &MockSummarizer).unwrap();

        // Force rebuild
        config.force = true;
        let summarizer = CountingSummarizer::new();
        let stats = build_with_summarizer(&config, &summarizer).unwrap();
        assert!(summarizer.call_count() > 0);
        assert_eq!(stats.skipped, 0);
    }

    #[test]
    fn test_sibling_records() {
        let tmp = setup_test_repo();
        let config = make_config(tmp.path().to_path_buf());

        build_with_summarizer(&config, &MockSummarizer).unwrap();

        // src directory should have a sibling record at root/.sem/src.md
        let sibling = records::record_path_for_dir_sibling(tmp.path(), "src");
        let record = records::read_record(&sibling).unwrap().unwrap();
        assert_eq!(record.node_type, "directory");
        assert_eq!(record.path, "src");
    }

    #[test]
    fn test_error_handling() {
        let tmp = setup_test_repo();
        let config = make_config(tmp.path().to_path_buf());

        let stats = build_with_summarizer(&config, &FailingSummarizer).unwrap();
        assert_eq!(stats.summarized, 0);
        assert!(stats.errored > 0);
    }

    #[test]
    fn test_oversized_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Create a file that's "oversized" with max_tokens=10
        // 100 bytes / 4 = 25 tokens > 10
        fs::write(root.join("big.txt"), "x".repeat(100)).unwrap();

        let mut config = make_config(root.to_path_buf());
        config.max_tokens = 10;

        let summarizer = CountingSummarizer::new();
        let stats = build_with_summarizer(&config, &summarizer).unwrap();

        // The file should be marked oversized, not summarized via the LLM
        // (1 summarized for the oversized file + 1 call for the root dir)
        // The oversized file doesn't call the summarizer
        let rec = records::record_path_for_file(root, "big.txt");
        let record = records::read_record(&rec).unwrap().unwrap();
        assert_eq!(record.summary, "summary unavailable: oversized file");

        // Root dir still gets summarized (1 LLM call for root dir only)
        assert_eq!(summarizer.call_count(), 1);
    }

    #[test]
    fn test_incremental_after_file_change() {
        let tmp = setup_test_repo();
        let config = make_config(tmp.path().to_path_buf());

        // First build
        let s1 = CountingSummarizer::new();
        build_with_summarizer(&config, &s1).unwrap();
        let first_count = s1.call_count();

        // Modify one file
        fs::write(tmp.path().join("README.md"), "# Updated Project").unwrap();

        // Second build: only changed file + affected directories should be re-summarized
        let s2 = CountingSummarizer::new();
        let stats = build_with_summarizer(&config, &s2).unwrap();
        // At minimum README.md + root dir should be re-summarized
        assert!(s2.call_count() > 0);
        assert!(s2.call_count() < first_count);
        assert!(stats.skipped > 0);
    }
}

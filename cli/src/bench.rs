use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use crate::records::{self, SEM_DIR};

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

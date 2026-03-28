use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const SEM_DIR: &str = ".sem";
pub const DIR_RECORD: &str = "__dir__.md";

/// A parsed .sem/ record with frontmatter fields and summary body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub path: String,
    pub node_type: String,
    pub content_hash: String,
    pub summary: String,
}

/// Frontmatter-only struct for YAML (de)serialization.
/// The `type` field in YAML maps to `node_type` here.
#[derive(Debug, Serialize, Deserialize)]
struct Frontmatter {
    path: String,
    #[serde(rename = "type")]
    node_type: String,
    content_hash: String,
}

/// Return the .sem/ record path for a file node.
///
/// For file `src/main.rs`, returns `<repo_root>/src/.sem/main.rs.md`.
pub fn record_path_for_file(repo_root: &Path, repo_relative: &str) -> PathBuf {
    let source = repo_root.join(repo_relative);
    let parent = source.parent().unwrap_or(repo_root);
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    parent.join(SEM_DIR).join(format!("{}.md", name))
}

/// Return the .sem/ record path for a directory node (the __dir__.md inside it).
///
/// For dir `src`, returns `<repo_root>/src/.sem/__dir__.md`.
/// For root (`""`), returns `<repo_root>/.sem/__dir__.md`.
pub fn record_path_for_dir(repo_root: &Path, repo_relative: &str) -> PathBuf {
    if repo_relative.is_empty() {
        repo_root.join(SEM_DIR).join(DIR_RECORD)
    } else {
        repo_root.join(repo_relative).join(SEM_DIR).join(DIR_RECORD)
    }
}

/// Return the .sem/ sibling record path for a directory node at its parent level.
///
/// For dir `src/auth`, returns `<repo_root>/src/.sem/auth.md`.
/// For root (`""` or `"."`), falls back to `<repo_root>/.sem/__dir__.md`.
pub fn record_path_for_dir_sibling(repo_root: &Path, repo_relative: &str) -> PathBuf {
    if repo_relative.is_empty() || repo_relative == "." {
        return repo_root.join(SEM_DIR).join(DIR_RECORD);
    }
    let source = repo_root.join(repo_relative);
    let parent = source.parent().unwrap_or(repo_root);
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    parent.join(SEM_DIR).join(format!("{}.md", name))
}

/// Write a .sem/ record with YAML frontmatter and Markdown body.
///
/// Creates parent directories as needed. Format:
/// ```text
/// ---
/// path: <path>
/// type: <node_type>
/// content_hash: <hash>
/// ---
///
/// <summary>
/// ```
pub fn write_record(
    record_file: &Path,
    path: &str,
    node_type: &str,
    content_hash: &str,
    summary: &str,
) -> Result<()> {
    if let Some(parent) = record_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let fm = Frontmatter {
        path: path.to_string(),
        node_type: node_type.to_string(),
        content_hash: content_hash.to_string(),
    };
    let fm_str = serde_yaml::to_string(&fm)?;
    // serde_yaml produces a trailing newline; trim it for consistent formatting
    let fm_str = fm_str.trim_end();

    let content = format!("---\n{}\n---\n\n{}\n", fm_str, summary);
    fs::write(record_file, content)?;
    Ok(())
}

/// Read a .sem/ record and return a parsed Record, or None if the file is
/// missing or malformed.
pub fn read_record(record_file: &Path) -> Result<Option<Record>> {
    if !record_file.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(record_file)?;
    let parts: Vec<&str> = text.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Ok(None);
    }

    let fm: Frontmatter = match serde_yaml::from_str(parts[1]) {
        Ok(fm) => fm,
        Err(_) => return Ok(None),
    };

    Ok(Some(Record {
        path: fm.path,
        node_type: fm.node_type,
        content_hash: fm.content_hash,
        summary: parts[2].trim().to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_record_path_for_file() {
        let root = PathBuf::from("/repo");
        let p = record_path_for_file(&root, "src/main.rs");
        assert_eq!(p, PathBuf::from("/repo/src/.sem/main.rs.md"));
    }

    #[test]
    fn test_record_path_for_file_top_level() {
        let root = PathBuf::from("/repo");
        let p = record_path_for_file(&root, "README.md");
        assert_eq!(p, PathBuf::from("/repo/.sem/README.md.md"));
    }

    #[test]
    fn test_record_path_for_dir_root() {
        let root = PathBuf::from("/repo");
        let p = record_path_for_dir(&root, "");
        assert_eq!(p, PathBuf::from("/repo/.sem/__dir__.md"));
    }

    #[test]
    fn test_record_path_for_dir_subdir() {
        let root = PathBuf::from("/repo");
        let p = record_path_for_dir(&root, "src/auth");
        assert_eq!(p, PathBuf::from("/repo/src/auth/.sem/__dir__.md"));
    }

    #[test]
    fn test_record_path_for_dir_sibling() {
        let root = PathBuf::from("/repo");
        let p = record_path_for_dir_sibling(&root, "src/auth");
        assert_eq!(p, PathBuf::from("/repo/src/.sem/auth.md"));
    }

    #[test]
    fn test_record_path_for_dir_sibling_root() {
        let root = PathBuf::from("/repo");
        let p = record_path_for_dir_sibling(&root, "");
        assert_eq!(p, PathBuf::from("/repo/.sem/__dir__.md"));

        let p2 = record_path_for_dir_sibling(&root, ".");
        assert_eq!(p2, PathBuf::from("/repo/.sem/__dir__.md"));
    }

    #[test]
    fn test_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let record_file = dir.path().join(".sem").join("test.rs.md");

        write_record(
            &record_file,
            "src/test.rs",
            "file",
            "abc123",
            "A test file that does things.",
        )
        .unwrap();

        let record = read_record(&record_file).unwrap().unwrap();
        assert_eq!(record.path, "src/test.rs");
        assert_eq!(record.node_type, "file");
        assert_eq!(record.content_hash, "abc123");
        assert_eq!(record.summary, "A test file that does things.");
    }

    #[test]
    fn test_read_missing_file_returns_none() {
        let p = PathBuf::from("/nonexistent/path/record.md");
        let result = read_record(&p).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_malformed_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let record_file = dir.path().join("bad.md");
        fs::write(&record_file, "no frontmatter here").unwrap();

        let result = read_record(&record_file).unwrap();
        assert!(result.is_none());
    }
}

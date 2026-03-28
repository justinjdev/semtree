//! Filesystem traversal for SRT construction.
//!
//! Post-order DFS: children are yielded before their parent directory.
//! Uses `git ls-files` when available to respect .gitignore and skip untracked files.
//! Falls back to filesystem walk for non-git repos.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use glob::Pattern;
use walkdir::WalkDir;

/// Directories always excluded (build artifacts, deps, caches).
const DEFAULT_EXCLUDE_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "dist",
    "build",
    "target",
    "third_party",
    "__pycache__",
    ".build",
    ".gradle",
    "_build",
    "deps",
    "_app",
    "immutable",
];

/// File suffixes always excluded (generated code, lock files, build output).
const DEFAULT_EXCLUDE_SUFFIXES: &[&str] = &[
    ".lock",
    ".sum",
    ".min.js",
    ".min.css",
    ".bundle.js",
    ".chunk.js",
    ".generated.go",
    "_generated.go",
    ".pb.go",
    ".gen.go",
    ".generated.ts",
    ".generated.js",
    "_pb2.py",
    "_pb2_grpc.py",
    ".pb.cc",
    ".pb.h",
    ".grpc.pb.cc",
    ".grpc.pb.h",
    ".d.ts",
];

/// Specific filenames always excluded.
const DEFAULT_EXCLUDE_FILES: &[&str] = &[
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "Cargo.lock",
    "Gemfile.lock",
    "poetry.lock",
    "composer.lock",
    "go.sum",
];

/// A node in the SRT filesystem tree.
#[derive(Debug, Clone)]
pub struct Node {
    /// Path relative to the repository root (empty string for root directory).
    pub repo_relative_path: String,
    /// Absolute path on disk.
    pub absolute_path: PathBuf,
    /// Whether this node represents a directory.
    pub is_directory: bool,
    /// Repo-relative paths of direct children (only populated for directories).
    pub children: Vec<String>,
}

/// Walk the repository in post-order DFS, yielding nodes bottom-up.
///
/// Uses `git ls-files` when in a git repo (respects .gitignore).
/// Falls back to filesystem walk otherwise.
pub fn walk(root: &Path, exclude: &[String]) -> Result<Vec<Node>> {
    let root = root.canonicalize()?;
    let exclude_patterns = compile_patterns(exclude);

    match git_tracked_files(&root) {
        Some(tracked) => build_tree_from_git(&root, &tracked, &exclude_patterns),
        None => build_tree_from_fs(&root, &exclude_patterns),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn compile_patterns(exclude: &[String]) -> Vec<Pattern> {
    exclude
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect()
}

fn matches_exclude(rel_path: &str, patterns: &[Pattern]) -> bool {
    for pat in patterns {
        if pat.matches(rel_path) {
            return true;
        }
        // Check partial paths (any ancestor directory matches)
        let p = Path::new(rel_path);
        let mut accum = PathBuf::new();
        for component in p.components() {
            accum.push(component);
            if pat.matches(accum.to_str().unwrap_or("")) {
                return true;
            }
        }
    }
    false
}

fn should_skip_file(rel_path: &str) -> bool {
    let name = Path::new(rel_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if DEFAULT_EXCLUDE_FILES.contains(&name) {
        return true;
    }
    for suffix in DEFAULT_EXCLUDE_SUFFIXES {
        if name.ends_with(suffix) {
            return true;
        }
    }
    false
}

fn should_skip_dir(name: &str) -> bool {
    DEFAULT_EXCLUDE_DIRS.contains(&name)
}

/// Check if a file is binary by looking for null bytes in the first 8KB.
fn is_binary(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return true;
    };
    let mut buf = [0u8; 8192];
    let n = match f.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return true,
    };
    buf[..n].contains(&0)
}

/// Return sorted list of git-tracked file paths, or None if not a git repo.
fn git_tracked_files(root: &Path) -> Option<Vec<String>> {
    let result = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
        .ok()?;

    if !result.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&result.stdout);
    let mut files: Vec<String> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();
    files.sort();
    Some(files)
}

/// Build post-order node list from git-tracked files.
fn build_tree_from_git(
    root: &Path,
    tracked_files: &[String],
    exclude: &[Pattern],
) -> Result<Vec<Node>> {
    // Filter files
    let mut valid_files: Vec<String> = Vec::new();
    for rel in tracked_files {
        let parts: Vec<&str> = Path::new(rel)
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        // Skip dotfiles/dot-directories
        if parts.iter().any(|p| p.starts_with('.')) {
            continue;
        }
        // Skip default-excluded directories (all parts except the last, which is the filename)
        if parts.len() > 1 && parts[..parts.len() - 1].iter().any(|p| should_skip_dir(p)) {
            continue;
        }
        if should_skip_file(rel) {
            continue;
        }
        if !exclude.is_empty() && matches_exclude(rel, exclude) {
            continue;
        }
        let fpath = root.join(rel);
        if !fpath.is_file() {
            continue;
        }
        if is_binary(&fpath) {
            continue;
        }
        valid_files.push(rel.clone());
    }

    let valid_set: HashSet<&str> = valid_files.iter().map(|s| s.as_str()).collect();

    // Build directory -> immediate children mapping
    let mut dir_children: HashMap<String, HashSet<String>> = HashMap::new();

    for rel in &valid_files {
        // Register file as child of its parent directory
        let parent = Path::new(rel)
            .parent()
            .map(|p| p.to_str().unwrap_or("").to_string())
            .unwrap_or_default();
        let parent_key = if parent == "." { String::new() } else { parent };
        dir_children
            .entry(parent_key)
            .or_default()
            .insert(rel.clone());

        // Register all ancestor directories
        let p = Path::new(rel);
        let components: Vec<&str> = p
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();
        for i in 0..components.len().saturating_sub(1) {
            let dir_path: PathBuf = components[..=i].iter().collect();
            let dir_str = dir_path.to_str().unwrap_or("").to_string();
            let parent_dir = if i > 0 {
                let pp: PathBuf = components[..i].iter().collect();
                pp.to_str().unwrap_or("").to_string()
            } else {
                String::new()
            };
            dir_children
                .entry(parent_dir)
                .or_default()
                .insert(dir_str);
        }
    }

    // Collect all directories
    let mut all_dirs: HashSet<String> = dir_children.keys().cloned().collect();
    for children in dir_children.values() {
        for c in children {
            if dir_children.contains_key(c) || root.join(c).is_dir() {
                all_dirs.insert(c.clone());
            }
        }
    }

    // Sort directories deepest-first for post-order
    let mut sorted_dirs: Vec<String> = all_dirs.into_iter().collect();
    sorted_dirs.sort_by(|a, b| {
        let depth_a = if a.is_empty() {
            -1i64
        } else {
            a.matches(std::path::MAIN_SEPARATOR).count() as i64
                + a.matches('/').count() as i64
        };
        let depth_b = if b.is_empty() {
            -1i64
        } else {
            b.matches(std::path::MAIN_SEPARATOR).count() as i64
                + b.matches('/').count() as i64
        };
        // Deeper first, then alphabetical
        depth_b.cmp(&depth_a).then(a.cmp(b))
    });

    let mut nodes: Vec<Node> = Vec::new();
    for dir_path in &sorted_dirs {
        let children: Vec<String> = dir_children
            .get(dir_path)
            .map(|s| {
                let mut v: Vec<String> = s.iter().cloned().collect();
                v.sort();
                v
            })
            .unwrap_or_default();

        let file_children: Vec<String> = children
            .iter()
            .filter(|c| valid_set.contains(c.as_str()))
            .cloned()
            .collect();
        let dir_child_paths: Vec<String> = children
            .iter()
            .filter(|c| !valid_set.contains(c.as_str()))
            .cloned()
            .collect();

        // Emit file nodes first (sorted)
        for rel in &file_children {
            nodes.push(Node {
                repo_relative_path: rel.clone(),
                absolute_path: root.join(rel),
                is_directory: false,
                children: vec![],
            });
        }

        // Emit directory node
        let abs = if dir_path.is_empty() {
            root.to_path_buf()
        } else {
            root.join(dir_path)
        };
        let mut all_children = file_children;
        all_children.extend(dir_child_paths);
        all_children.sort();

        nodes.push(Node {
            repo_relative_path: dir_path.clone(),
            absolute_path: abs,
            is_directory: true,
            children: all_children,
        });
    }

    Ok(nodes)
}

/// Build post-order node list from filesystem walk (non-git fallback).
fn build_tree_from_fs(root: &Path, exclude: &[Pattern]) -> Result<Vec<Node>> {
    // Collect directory entries using os walk equivalent
    // We'll use walkdir but process in a way that mimics os.walk with top-down filtering
    struct DirEntry {
        path: PathBuf,
        subdirs: Vec<String>,
        files: Vec<String>,
    }

    let mut entries: Vec<DirEntry> = Vec::new();

    // Walk using walkdir with max_depth=1 at each level, implementing our own recursion
    // to properly filter subdirs top-down. Simpler: just use walkdir and group.
    fn collect_entries(
        dir: &Path,
        root: &Path,
        exclude: &[Pattern],
        entries: &mut Vec<DirEntry>,
    ) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };

        let mut subdirs: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();

        let mut dir_paths: Vec<PathBuf> = Vec::new();

        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if ft.is_symlink() {
                continue;
            }

            if name.starts_with('.') {
                continue;
            }

            if ft.is_dir() {
                if should_skip_dir(&name) {
                    continue;
                }
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                if !exclude.is_empty() && matches_exclude(&rel, exclude) {
                    continue;
                }
                subdirs.push(name);
                dir_paths.push(path);
            } else if ft.is_file() {
                files.push(name);
            }
        }

        subdirs.sort();
        dir_paths.sort();

        // Recurse into subdirs first (so deeper entries come first when reversed)
        for sub_path in &dir_paths {
            collect_entries(sub_path, root, exclude, entries);
        }

        entries.push(DirEntry {
            path: dir.to_path_buf(),
            subdirs,
            files,
        });
    }

    collect_entries(root, root, exclude, &mut entries);

    let mut nodes: Vec<Node> = Vec::new();
    for entry in &entries {
        let mut child_paths: Vec<String> = Vec::new();

        let mut sorted_files = entry.files.clone();
        sorted_files.sort();

        for fname in &sorted_files {
            let fpath = entry.path.join(fname);

            if fname.starts_with('.') || fpath.is_symlink() {
                continue;
            }

            let rel = fpath
                .strip_prefix(root)
                .unwrap_or(&fpath)
                .to_str()
                .unwrap_or("")
                .to_string();

            if should_skip_file(&rel) {
                continue;
            }
            if !exclude.is_empty() && matches_exclude(&rel, exclude) {
                continue;
            }
            if is_binary(&fpath) {
                continue;
            }

            child_paths.push(rel.clone());
            nodes.push(Node {
                repo_relative_path: rel,
                absolute_path: fpath,
                is_directory: false,
                children: vec![],
            });
        }

        for dname in &entry.subdirs {
            let rel = entry
                .path
                .join(dname)
                .strip_prefix(root)
                .unwrap_or(Path::new(dname))
                .to_str()
                .unwrap_or("")
                .to_string();
            child_paths.push(rel);
        }

        let rel_dir = entry
            .path
            .strip_prefix(root)
            .unwrap_or(Path::new(""))
            .to_str()
            .unwrap_or("")
            .to_string();
        let rel_dir = if rel_dir == "." { String::new() } else { rel_dir };

        child_paths.sort();

        nodes.push(Node {
            repo_relative_path: rel_dir,
            absolute_path: entry.path.clone(),
            is_directory: true,
            children: child_paths,
        });
    }

    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a temporary directory tree for testing.
    fn setup_test_dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Create directory structure:
        //   root/
        //     a.txt
        //     sub/
        //       b.txt
        //       deep/
        //         c.txt
        //     .hidden_file
        //     .hidden_dir/
        //       x.txt
        fs::create_dir_all(root.join("sub/deep")).unwrap();
        fs::create_dir_all(root.join(".hidden_dir")).unwrap();

        fs::write(root.join("a.txt"), "hello").unwrap();
        fs::write(root.join("sub/b.txt"), "world").unwrap();
        fs::write(root.join("sub/deep/c.txt"), "nested").unwrap();
        fs::write(root.join(".hidden_file"), "secret").unwrap();
        fs::write(root.join(".hidden_dir/x.txt"), "hidden").unwrap();

        tmp
    }

    #[test]
    fn test_post_order_children_before_parents() {
        let tmp = setup_test_dir();
        let nodes = build_tree_from_fs(tmp.path(), &[]).unwrap();

        // Find positions of directory nodes and their children
        let positions: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.repo_relative_path.as_str(), i))
            .collect();

        // Files should appear before their parent directory
        assert!(positions["sub/deep/c.txt"] < positions["sub/deep"]);
        assert!(positions["sub/b.txt"] < positions["sub"]);
        assert!(positions["a.txt"] < positions[""]);

        // Deeper directories should appear before shallower ones
        assert!(positions["sub/deep"] < positions["sub"]);
        assert!(positions["sub"] < positions[""]);
    }

    #[test]
    fn test_dotfile_filtering() {
        let tmp = setup_test_dir();
        let nodes = build_tree_from_fs(tmp.path(), &[]).unwrap();

        let paths: Vec<&str> = nodes
            .iter()
            .map(|n| n.repo_relative_path.as_str())
            .collect();

        // Dotfiles and dot-directories should be excluded
        assert!(!paths.contains(&".hidden_file"));
        assert!(!paths.contains(&".hidden_dir"));
        assert!(!paths.contains(&".hidden_dir/x.txt"));
    }

    #[test]
    fn test_binary_detection() {
        let tmp = tempfile::tempdir().unwrap();

        // Text file
        let text_path = tmp.path().join("text.txt");
        fs::write(&text_path, "hello world\n").unwrap();
        assert!(!is_binary(&text_path));

        // Binary file (contains null bytes)
        let bin_path = tmp.path().join("binary.bin");
        fs::write(&bin_path, b"hello\x00world").unwrap();
        assert!(is_binary(&bin_path));

        // Empty file is not binary
        let empty_path = tmp.path().join("empty.txt");
        fs::write(&empty_path, b"").unwrap();
        assert!(!is_binary(&empty_path));
    }

    #[test]
    fn test_exclude_patterns() {
        let tmp = setup_test_dir();
        let exclude = vec![Pattern::new("sub/deep/*").unwrap()];
        let nodes = build_tree_from_fs(tmp.path(), &exclude).unwrap();

        let paths: Vec<&str> = nodes
            .iter()
            .map(|n| n.repo_relative_path.as_str())
            .collect();

        assert!(!paths.contains(&"sub/deep/c.txt"));
        assert!(paths.contains(&"sub/b.txt"));
        assert!(paths.contains(&"a.txt"));
    }

    #[test]
    fn test_default_exclude_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("package-lock.json"), "{}").unwrap();
        fs::write(tmp.path().join("index.js"), "code").unwrap();

        let nodes = build_tree_from_fs(tmp.path(), &[]).unwrap();
        let paths: Vec<&str> = nodes
            .iter()
            .map(|n| n.repo_relative_path.as_str())
            .collect();

        assert!(!paths.contains(&"package-lock.json"));
        assert!(paths.contains(&"index.js"));
    }

    #[test]
    fn test_default_exclude_suffixes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("app.min.js"), "minified").unwrap();
        fs::write(tmp.path().join("types.d.ts"), "declare").unwrap();
        fs::write(tmp.path().join("app.js"), "code").unwrap();

        let nodes = build_tree_from_fs(tmp.path(), &[]).unwrap();
        let paths: Vec<&str> = nodes
            .iter()
            .map(|n| n.repo_relative_path.as_str())
            .collect();

        assert!(!paths.contains(&"app.min.js"));
        assert!(!paths.contains(&"types.d.ts"));
        assert!(paths.contains(&"app.js"));
    }

    #[test]
    fn test_directory_children_populated() {
        let tmp = setup_test_dir();
        let nodes = build_tree_from_fs(tmp.path(), &[]).unwrap();

        let sub_node = nodes
            .iter()
            .find(|n| n.repo_relative_path == "sub")
            .expect("sub directory node should exist");

        assert!(sub_node.is_directory);
        assert!(sub_node.children.contains(&"sub/b.txt".to_string()));
        assert!(sub_node.children.contains(&"sub/deep".to_string()));
    }

    #[test]
    fn test_file_nodes_have_no_children() {
        let tmp = setup_test_dir();
        let nodes = build_tree_from_fs(tmp.path(), &[]).unwrap();

        for node in &nodes {
            if !node.is_directory {
                assert!(
                    node.children.is_empty(),
                    "file node {} should have no children",
                    node.repo_relative_path
                );
            }
        }
    }

    #[test]
    fn test_default_exclude_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("node_modules")).unwrap();
        fs::write(tmp.path().join("node_modules/pkg.js"), "module").unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/app.js"), "code").unwrap();

        let nodes = build_tree_from_fs(tmp.path(), &[]).unwrap();
        let paths: Vec<&str> = nodes
            .iter()
            .map(|n| n.repo_relative_path.as_str())
            .collect();

        assert!(!paths.contains(&"node_modules"));
        assert!(!paths.contains(&"node_modules/pkg.js"));
        assert!(paths.contains(&"src/app.js"));
    }
}

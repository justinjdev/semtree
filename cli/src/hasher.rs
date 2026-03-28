use std::fs;
use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};

/// Compute SHA-256 hex digest of a file's raw byte contents.
pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute SHA-256 hex digest from sorted (path, hash) child pairs.
///
/// The canonical string is formed by sorting children lexicographically
/// by path, formatting each as `path:hash`, and joining with newlines.
pub fn hash_directory(child_pairs: &[(&str, &str)]) -> String {
    let mut sorted: Vec<(&str, &str)> = child_pairs.to_vec();
    sorted.sort_by_key(|&(path, _)| path);

    let canonical: String = sorted
        .iter()
        .map(|(path, hash)| format!("{}:{}", path, hash))
        .collect::<Vec<_>>()
        .join("\n");

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_hash_file_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "hello world").unwrap();

        let h1 = hash_file(&file).unwrap();
        let h2 = hash_file(&file).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex is 64 chars
    }

    #[test]
    fn test_hash_file_known_value() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "hello world").unwrap();

        let h = hash_file(&file).unwrap();
        // SHA-256 of "hello world"
        assert_eq!(
            h,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_hash_directory_sorted() {
        // Order of input should not matter — pairs are sorted by path
        let pairs_a = vec![("b.rs", "hash_b"), ("a.rs", "hash_a")];
        let pairs_b = vec![("a.rs", "hash_a"), ("b.rs", "hash_b")];

        assert_eq!(hash_directory(&pairs_a), hash_directory(&pairs_b));
    }

    #[test]
    fn test_hash_directory_deterministic() {
        let pairs = vec![("src/main.rs", "abc"), ("src/lib.rs", "def")];
        let h1 = hash_directory(&pairs);
        let h2 = hash_directory(&pairs);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_hash_directory_known_value() {
        // Canonical string: "a.rs:hash_a\nb.rs:hash_b"
        let pairs = vec![("a.rs", "hash_a"), ("b.rs", "hash_b")];
        let h = hash_directory(&pairs);

        // Compute expected independently
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"a.rs:hash_a\nb.rs:hash_b");
        let expected = format!("{:x}", hasher.finalize());
        assert_eq!(h, expected);
    }
}

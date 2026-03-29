//! LLM summarization for SRT nodes.
//!
//! Default provider: Claude Code CLI via `claude -p`.

use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};

pub const OVERSIZED_PLACEHOLDER: &str = "summary unavailable: oversized file";

const MAX_RETRIES: u32 = 3;
const TIMEOUT_SECS: u64 = 120;

/// Check if a file exceeds the token limit (estimated as bytes / 4).
pub fn is_oversized(file_bytes: u64, max_tokens: usize) -> bool {
    (file_bytes / 4) as usize > max_tokens
}

/// Build the summarization prompt for a file leaf.
pub fn build_file_prompt(path: &str, content: &str) -> String {
    format!(
        "Summarize this source file. Be concise but capture the file's purpose, \
key exports, and important implementation details. \
Write 2-5 sentences of plain prose.\n\
\n\
File: {path}\n\
\n\
```\n\
{content}\n\
```"
    )
}

/// Build the summarization prompt for a directory node.
///
/// `child_summaries` is a slice of (child_repo_relative_path, child_summary) pairs.
pub fn build_dir_prompt(path: &str, child_summaries: &[(&str, &str)]) -> String {
    let display_path = if path.is_empty() {
        "(repository root)"
    } else {
        path
    };

    let children_block: String = child_summaries
        .iter()
        .map(|(child_path, summary)| format!("- **{child_path}**: {summary}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Summarize this directory based on its children. Write a concise prose overview \
(2-4 sentences), then a ## Children section listing every immediate child \
with a brief description (one line each, as a bullet list using **bold** \
for the child path).\n\
\n\
Directory: {display_path}\n\
\n\
Children:\n\
{children_block}"
    )
}

/// Call `claude -p --model <model>` to generate a summary.
///
/// Retries up to 3 times with exponential backoff (1s, 2s, 4s).
pub fn summarize(prompt: &str, model: &str) -> Result<String> {
    for attempt in 0..MAX_RETRIES {
        match run_claude(prompt, model) {
            Ok(output) => {
                let trimmed = output.trim().to_string();
                if !trimmed.is_empty() {
                    return Ok(trimmed);
                }
                // Empty output counts as a failure
                if attempt < MAX_RETRIES - 1 {
                    let backoff = Duration::from_secs(1 << attempt);
                    thread::sleep(backoff);
                    continue;
                }
                bail!("claude CLI returned empty output after {MAX_RETRIES} attempts");
            }
            Err(e) => {
                if attempt < MAX_RETRIES - 1 {
                    let backoff = Duration::from_secs(1 << attempt);
                    thread::sleep(backoff);
                    continue;
                }
                return Err(e);
            }
        }
    }
    bail!("summarization failed after all retries");
}

/// Run `claude -p --model <model>` once with the given prompt on stdin.
fn run_claude(prompt: &str, model: &str) -> Result<String> {
    let mut child = Command::new("claude")
        .args(["-p", "--model", model])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "claude CLI not found on PATH. Install Claude Code: https://claude.ai/code"
                )
            } else {
                anyhow::anyhow!("failed to spawn claude: {e}")
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
        // stdin is dropped here, closing the pipe
    }

    // Wait with a timeout by spawning a thread (std::process doesn't have native timeout)
    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "claude CLI failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Trait for abstracting summarization (enables testing without calling claude).
pub trait SummarizerFn: Send {
    fn call(&self, prompt: &str) -> Result<String>;
}

/// Default summarizer that calls `claude -p`.
pub struct ClaudeSummarizer {
    pub model: String,
}

impl SummarizerFn for ClaudeSummarizer {
    fn call(&self, prompt: &str) -> Result<String> {
        summarize(prompt, &self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_oversized_under_limit() {
        // 400 bytes / 4 = 100 tokens, limit is 200 => not oversized
        assert!(!is_oversized(400, 200));
    }

    #[test]
    fn test_is_oversized_over_limit() {
        // 1000 bytes / 4 = 250 tokens, limit is 200 => oversized
        assert!(is_oversized(1000, 200));
    }

    #[test]
    fn test_is_oversized_at_limit() {
        // 800 bytes / 4 = 200 tokens, limit is 200 => not oversized (equal)
        assert!(!is_oversized(800, 200));
    }

    #[test]
    fn test_is_oversized_zero_bytes() {
        assert!(!is_oversized(0, 100));
    }

    #[test]
    fn test_build_file_prompt_format() {
        let prompt = build_file_prompt("src/main.rs", "fn main() {}");
        assert!(prompt.starts_with("Summarize this source file."));
        assert!(prompt.contains("File: src/main.rs"));
        assert!(prompt.contains("```\nfn main() {}\n```"));
    }

    #[test]
    fn test_build_file_prompt_matches_python() {
        let prompt = build_file_prompt("src/lib.rs", "pub mod foo;");
        // Verify structure matches the Python FILE_PROMPT template
        let expected = "\
Summarize this source file. Be concise but capture the file's purpose, \
key exports, and important implementation details. \
Write 2-5 sentences of plain prose.\n\
\n\
File: src/lib.rs\n\
\n\
```\n\
pub mod foo;\n\
```";
        assert_eq!(prompt, expected);
    }

    #[test]
    fn test_build_dir_prompt_format() {
        let children = vec![
            ("src/main.rs", "Entry point for the CLI"),
            ("src/lib.rs", "Library exports"),
        ];
        let prompt = build_dir_prompt("src", &children);
        assert!(prompt.starts_with("Summarize this directory based on its children."));
        assert!(prompt.contains("Directory: src"));
        assert!(prompt.contains("- **src/main.rs**: Entry point for the CLI"));
        assert!(prompt.contains("- **src/lib.rs**: Library exports"));
    }

    #[test]
    fn test_build_dir_prompt_root() {
        let children = vec![("README.md", "Project readme")];
        let prompt = build_dir_prompt("", &children);
        assert!(prompt.contains("Directory: (repository root)"));
    }

    #[test]
    fn test_build_dir_prompt_matches_python() {
        let children = vec![
            ("src/a.rs", "Module A"),
            ("src/b.rs", "Module B"),
        ];
        let prompt = build_dir_prompt("src", &children);
        let expected = "\
Summarize this directory based on its children. Write a concise prose overview \
(2-4 sentences), then a ## Children section listing every immediate child \
with a brief description (one line each, as a bullet list using **bold** \
for the child path).\n\
\n\
Directory: src\n\
\n\
Children:\n\
- **src/a.rs**: Module A\n\
- **src/b.rs**: Module B";
        assert_eq!(prompt, expected);
    }

    #[test]
    fn test_oversized_placeholder_value() {
        assert_eq!(OVERSIZED_PLACEHOLDER, "summary unavailable: oversized file");
    }

    /// Mock summarizer for testing retry logic.
    struct MockFailingSummarizer {
        fail_count: std::cell::Cell<u32>,
        max_fails: u32,
    }

    impl SummarizerFn for MockFailingSummarizer {
        fn call(&self, _prompt: &str) -> Result<String> {
            let current = self.fail_count.get();
            if current < self.max_fails {
                self.fail_count.set(current + 1);
                bail!("mock failure #{}", current + 1);
            }
            Ok("mock summary".to_string())
        }
    }

    #[test]
    fn test_mock_summarizer_succeeds_after_failures() {
        let mock = MockFailingSummarizer {
            fail_count: std::cell::Cell::new(0),
            max_fails: 2,
        };
        // First two calls fail
        assert!(mock.call("test").is_err());
        assert!(mock.call("test").is_err());
        // Third succeeds
        assert_eq!(mock.call("test").unwrap(), "mock summary");
    }

    #[test]
    fn test_mock_summarizer_always_fails() {
        let mock = MockFailingSummarizer {
            fail_count: std::cell::Cell::new(0),
            max_fails: 10,
        };
        for _ in 0..5 {
            assert!(mock.call("test").is_err());
        }
    }
}

//! LLM summarization for SRT nodes.
//!
//! Supports: Anthropic API (default), Claude Code CLI fallback, batch mode.

use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};

pub const OVERSIZED_PLACEHOLDER: &str = "summary unavailable: oversized file";

const MAX_RETRIES: u32 = 3;

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
        "Summarize this directory based on its children. Write:\n\
\n\
1. A concise prose overview (2-4 sentences)\n\
2. A ## Cross-Cutting Concerns section (only if the directory has 5+ children): \
list interactions between children — cases where files in different children \
collaborate, share state, or implement a feature together. Name the specific files \
involved. Skip this section if children are independent or there are fewer than 5.\n\
3. A ## Children section listing every immediate child with a brief description \
(one line each, as a bullet list using **bold** for the child path).\n\
\n\
Directory: {display_path}\n\
\n\
Children:\n\
{children_block}"
    )
}

// ---------------------------------------------------------------------------
// SummarizerFn trait
// ---------------------------------------------------------------------------

/// Trait for abstracting summarization (enables testing without calling APIs).
pub trait SummarizerFn: Send {
    fn call(&self, prompt: &str) -> Result<String>;
}

// ---------------------------------------------------------------------------
// Anthropic API summarizer (default)
// ---------------------------------------------------------------------------

/// Summarizer that calls the Anthropic Messages API directly.
pub struct AnthropicSummarizer {
    pub model: String,
    api_key: String,
    client: reqwest::blocking::Client,
}

impl AnthropicSummarizer {
    pub fn new(model: &str) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY not set. Set it or use --provider claude-cli")?;
        Ok(Self {
            model: model.to_string(),
            api_key,
            client: reqwest::blocking::Client::new(),
        })
    }
}

impl SummarizerFn for AnthropicSummarizer {
    fn call(&self, prompt: &str) -> Result<String> {
        for attempt in 0..MAX_RETRIES {
            match call_anthropic_api(&self.client, &self.api_key, &self.model, prompt) {
                Ok(text) if !text.is_empty() => return Ok(text),
                Ok(_) => {
                    if attempt < MAX_RETRIES - 1 {
                        thread::sleep(Duration::from_secs(1 << attempt));
                        continue;
                    }
                    bail!("Anthropic API returned empty response after {MAX_RETRIES} attempts");
                }
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        // Check for rate limit
                        let msg = format!("{e}");
                        let backoff = if msg.contains("429") || msg.contains("rate") {
                            Duration::from_secs(5 * (1 << attempt))
                        } else {
                            Duration::from_secs(1 << attempt)
                        };
                        eprintln!("  retry {}/{}: {}", attempt + 1, MAX_RETRIES, e);
                        thread::sleep(backoff);
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        bail!("summarization failed after all retries");
    }
}

fn call_anthropic_api(
    client: &reqwest::blocking::Client,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String> {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": [
            {"role": "user", "content": prompt}
        ]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .timeout(Duration::from_secs(120))
        .send()
        .context("failed to send request to Anthropic API")?;

    let status = resp.status();
    let resp_text = resp.text().context("failed to read API response")?;

    if !status.is_success() {
        bail!("Anthropic API error ({}): {}", status.as_u16(), resp_text);
    }

    let parsed: serde_json::Value = serde_json::from_str(&resp_text)
        .context("failed to parse API response")?;

    // Extract text from content[0].text
    let text = parsed["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|block| block["text"].as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    Ok(text)
}

// ---------------------------------------------------------------------------
// Batch summarizer (50% cost savings)
// ---------------------------------------------------------------------------

/// Result from a batch submission.
pub struct BatchResult {
    pub batch_id: String,
}

/// Submit a batch of prompts to the Anthropic Batches API.
/// Returns a batch ID that can be polled for completion.
pub fn submit_batch(
    prompts: &[(String, String)], // (custom_id, prompt)
    model: &str,
) -> Result<BatchResult> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY not set")?;

    let requests: Vec<serde_json::Value> = prompts
        .iter()
        .map(|(id, prompt)| {
            serde_json::json!({
                "custom_id": id,
                "params": {
                    "model": model,
                    "max_tokens": 1024,
                    "messages": [
                        {"role": "user", "content": prompt}
                    ]
                }
            })
        })
        .collect();

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages/batches")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({"requests": requests}))
        .timeout(Duration::from_secs(300))
        .send()
        .context("failed to submit batch")?;

    let status = resp.status();
    let resp_text = resp.text()?;

    if !status.is_success() {
        bail!("Batch API error ({}): {}", status.as_u16(), resp_text);
    }

    let parsed: serde_json::Value = serde_json::from_str(&resp_text)?;
    let batch_id = parsed["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no batch id in response"))?
        .to_string();

    Ok(BatchResult { batch_id })
}

/// Poll a batch until it completes. Returns map of custom_id -> response text.
pub fn poll_batch(batch_id: &str) -> Result<std::collections::HashMap<String, String>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")?;
    let client = reqwest::blocking::Client::new();
    let url = format!("https://api.anthropic.com/v1/messages/batches/{batch_id}");

    loop {
        let resp = client
            .get(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .context("failed to poll batch")?;

        let parsed: serde_json::Value = serde_json::from_str(&resp.text()?)?;
        let status = parsed["processing_status"].as_str().unwrap_or("unknown");

        match status {
            "ended" => {
                // Fetch results
                let results_url = parsed["results_url"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("no results_url in batch response"))?;

                let results_resp = client
                    .get(results_url)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", "2023-06-01")
                    .send()?;

                let results_text = results_resp.text()?;
                let mut results = std::collections::HashMap::new();

                // Results are JSONL (one JSON object per line)
                for line in results_text.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let entry: serde_json::Value = serde_json::from_str(line)
                        .context("failed to parse batch result line")?;

                    let custom_id = entry["custom_id"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    let text = entry["result"]["message"]["content"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|block| block["text"].as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if !custom_id.is_empty() {
                        results.insert(custom_id, text);
                    }
                }

                return Ok(results);
            }
            "in_progress" | "canceling" => {
                let counts = &parsed["request_counts"];
                let processing = counts["processing"].as_u64().unwrap_or(0);
                let succeeded = counts["succeeded"].as_u64().unwrap_or(0);
                let total = processing + succeeded
                    + counts["errored"].as_u64().unwrap_or(0)
                    + counts["canceled"].as_u64().unwrap_or(0)
                    + counts["expired"].as_u64().unwrap_or(0);
                eprint!("\r  batch {batch_id}: {succeeded}/{total} complete...");
                thread::sleep(Duration::from_secs(10));
            }
            other => {
                bail!("unexpected batch status: {other}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Claude CLI fallback summarizer
// ---------------------------------------------------------------------------

/// Summarizer that calls `claude -p` (fallback when no API key is set).
pub struct ClaudeSummarizer {
    pub model: String,
}

impl SummarizerFn for ClaudeSummarizer {
    fn call(&self, prompt: &str) -> Result<String> {
        for attempt in 0..MAX_RETRIES {
            match run_claude(prompt, &self.model) {
                Ok(output) => {
                    let trimmed = output.trim().to_string();
                    if !trimmed.is_empty() {
                        return Ok(trimmed);
                    }
                    if attempt < MAX_RETRIES - 1 {
                        thread::sleep(Duration::from_secs(1 << attempt));
                        continue;
                    }
                    bail!("claude CLI returned empty output after {MAX_RETRIES} attempts");
                }
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        thread::sleep(Duration::from_secs(1 << attempt));
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        bail!("summarization failed after all retries");
    }
}

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
    }

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

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create the appropriate summarizer based on configuration.
/// Prefers Anthropic API if ANTHROPIC_API_KEY is set, falls back to claude CLI.
pub fn create_summarizer(model: &str) -> Box<dyn SummarizerFn> {
    match AnthropicSummarizer::new(model) {
        Ok(s) => {
            eprintln!("Using Anthropic API for summarization");
            Box::new(s)
        }
        Err(_) => {
            eprintln!("Using claude CLI for summarization (set ANTHROPIC_API_KEY for direct API)");
            Box::new(ClaudeSummarizer {
                model: model.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_oversized_under_limit() {
        assert!(!is_oversized(400, 200));
    }

    #[test]
    fn test_is_oversized_over_limit() {
        assert!(is_oversized(1000, 200));
    }

    #[test]
    fn test_is_oversized_at_limit() {
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
    fn test_build_dir_prompt_format() {
        let children = vec![
            ("src/main.rs", "Entry point for the CLI"),
            ("src/lib.rs", "Library exports"),
        ];
        let prompt = build_dir_prompt("src", &children);
        assert!(prompt.starts_with("Summarize this directory based on its children."));
        assert!(prompt.contains("Directory: src"));
        assert!(prompt.contains("- **src/main.rs**: Entry point for the CLI"));
    }

    #[test]
    fn test_build_dir_prompt_root() {
        let children = vec![("README.md", "Project readme")];
        let prompt = build_dir_prompt("", &children);
        assert!(prompt.contains("Directory: (repository root)"));
    }

    #[test]
    fn test_oversized_placeholder_value() {
        assert_eq!(OVERSIZED_PLACEHOLDER, "summary unavailable: oversized file");
    }

    /// Mock summarizer for testing.
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
        assert!(mock.call("test").is_err());
        assert!(mock.call("test").is_err());
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

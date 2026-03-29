## ADDED Requirements

### Requirement: Claude CLI subprocess invocation
The summarizer SHALL invoke `claude -p` as a subprocess, passing the prompt text on stdin and capturing stdout as the summary result. The `--model` flag SHALL be forwarded to the claude CLI invocation.

#### Scenario: File summarization via claude CLI
- **WHEN** the summarizer is called for a file
- **THEN** it spawns `claude -p --model <model>` with the prompt on stdin and returns the captured stdout as the summary

#### Scenario: Model flag forwarded
- **WHEN** the summarizer is configured with model `claude-haiku-4-5-20251001`
- **THEN** the subprocess is invoked as `claude -p --model claude-haiku-4-5-20251001`

### Requirement: Retry with exponential backoff
The summarizer SHALL retry failed subprocess calls up to 3 times with exponential backoff. The backoff delays SHALL be 1 second, 2 seconds, and 4 seconds for the first, second, and third retries respectively.

#### Scenario: Transient failure with successful retry
- **WHEN** the claude CLI returns a non-zero exit code on the first attempt but succeeds on the second
- **THEN** the summarizer waits 1 second, retries, and returns the successful result

#### Scenario: All retries exhausted
- **WHEN** the claude CLI fails on all 4 attempts (initial + 3 retries)
- **THEN** the summarizer returns an error indicating the summarization failed after retries

#### Scenario: Exponential backoff timing
- **WHEN** the claude CLI fails 3 times before succeeding on the 4th attempt
- **THEN** the delays between attempts are approximately 1s, 2s, and 4s

### Requirement: Oversized file detection
The summarizer SHALL detect oversized files before making an LLM call. A file is oversized when `file_bytes_length / 4 > max_tokens`. Oversized files SHALL receive the placeholder summary `summary unavailable: oversized file` without invoking the LLM.

#### Scenario: Oversized file gets placeholder
- **WHEN** the summarizer is called for a 500KB file with max_tokens set to 100000
- **THEN** since 500000 / 4 = 125000 > 100000, no LLM call is made and the summary is `summary unavailable: oversized file`

#### Scenario: File within token limit is summarized normally
- **WHEN** the summarizer is called for a 10KB file with max_tokens set to 100000
- **THEN** since 10000 / 4 = 2500 < 100000, the file is sent to the LLM for summarization

### Requirement: File prompt format
The file summarization prompt SHALL include the repo-relative path and the full file contents. The prompt MUST provide sufficient context for the LLM to generate a useful routing summary.

#### Scenario: File prompt includes path and contents
- **WHEN** the summarizer generates a prompt for `src/auth/login.rs`
- **THEN** the prompt contains the repo-relative path `src/auth/login.rs` and the complete file contents

### Requirement: Directory prompt format
The directory summarization prompt SHALL include the repo-relative path and a routing table of immediate children with their summaries. The prompt MUST instruct the LLM to produce a summary with a `## Children` section listing every child.

#### Scenario: Directory prompt includes children routing table
- **WHEN** the summarizer generates a prompt for directory `src/auth/` with children `login.rs` and `session.rs`
- **THEN** the prompt contains the directory path and each child's name paired with its summary

#### Scenario: Directory prompt requests Children section
- **WHEN** the summarizer generates a directory prompt
- **THEN** the prompt instructs the LLM to include a `## Children` section mentioning every immediate child

### Requirement: Configurable model
The summarizer SHALL accept a model name parameter, defaulting to `claude-sonnet-4-20250514`. The model name is passed to the claude CLI via the `--model` flag.

#### Scenario: Default model used
- **WHEN** the summarizer is invoked without a model override
- **THEN** the claude CLI is called with `--model claude-sonnet-4-20250514`

#### Scenario: Custom model specified
- **WHEN** the summarizer is invoked with model `claude-haiku-4-5-20251001`
- **THEN** the claude CLI is called with `--model claude-haiku-4-5-20251001`

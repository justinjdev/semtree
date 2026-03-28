## ADDED Requirements

### Requirement: File summarization via LLM
The indexer SHALL generate a natural-language summary for each included file by sending the repo-relative path and full file contents to an LLM. The prompt SHALL be minimal: the path and contents are the input, and the instruction is to summarize the file.

#### Scenario: File summary generated
- **WHEN** the indexer processes a text file that fits within the token limit
- **THEN** an LLM call is made with the file's repo-relative path and full contents, and the returned summary is stored in the file's `.sem/` record

#### Scenario: Prompt includes repo-relative path
- **WHEN** the indexer summarizes `src/auth/login.py`
- **THEN** the LLM prompt includes the path `src/auth/login.py` as context for the summary

### Requirement: Directory summarization via LLM
The indexer SHALL generate a directory summary by sending the list of immediate child `(path, summary)` pairs to an LLM. The directory summary SHALL include a `## Children` routing table that mentions every immediate child by name with a brief description.

#### Scenario: Directory summary generated from children
- **WHEN** the indexer processes a directory whose children have all been summarized
- **THEN** an LLM call is made with the list of child path/summary pairs, and the returned summary includes a `## Children` section listing every immediate child

#### Scenario: Every child mentioned in routing table
- **WHEN** a directory has children `auth.py`, `config.py`, and `utils/`
- **THEN** the directory's `## Children` section mentions all three by name

### Requirement: Oversized file handling
The indexer SHALL skip LLM summarization for files whose estimated token count exceeds a configurable maximum. The token estimate SHALL be `len(file_bytes) / 4`. Oversized files SHALL receive the placeholder summary `summary unavailable: oversized file`.

#### Scenario: Oversized file gets placeholder
- **WHEN** the indexer encounters a file whose `len(bytes) / 4` exceeds the configured max tokens
- **THEN** no LLM call is made and the record's summary is `summary unavailable: oversized file`

#### Scenario: Oversized file still appears in parent routing table
- **WHEN** a directory contains an oversized file
- **THEN** the parent directory's summary still mentions the oversized file by path with the placeholder marker

### Requirement: LLM provider interface
The indexer SHALL define a summarization interface that accepts a prompt string and content string and returns a summary string. The default implementation SHALL use the `claude` CLI in pipe mode (`claude -p`), requiring no API key configuration from the user.

#### Scenario: Default provider uses claude CLI
- **WHEN** the indexer is run without specifying a custom provider
- **THEN** summarization calls are made via `claude -p` subprocess invocations

#### Scenario: Claude CLI not found
- **WHEN** the `claude` CLI is not available on PATH
- **THEN** the indexer exits with a clear error message indicating that Claude Code CLI is required

### Requirement: Summarization error handling
The indexer SHALL handle LLM call failures gracefully. If a summarization call fails after retries, the indexer SHALL log the error and skip that node, continuing with the rest of the build.

#### Scenario: Transient API failure with retry
- **WHEN** an LLM call fails on the first attempt but succeeds on retry
- **THEN** the summary is stored normally

#### Scenario: Persistent failure skips node
- **WHEN** an LLM call fails after all retries
- **THEN** the node is skipped, an error is logged, and the build continues with remaining nodes
